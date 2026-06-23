use std::ffi::c_void;
use vulkanite::{vk, DefaultAllocator, Dispatcher};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vk_dispatcher = unsafe { vulkanite::DynamicDispatcher::new_loaded() }?;
    let vk_entry = vk::rs::Entry::new(vk_dispatcher, DefaultAllocator);
    let app_info = vk::ApplicationInfo::default()
        .api_version(vk::ApiVersion::new(0, 1, 4, 0));

    let instance_create_info = vk::InstanceCreateInfo::default()
        .application_info(Some(&app_info));

    let instance = vk_entry.create_instance(&instance_create_info)?;

    let physical_devices: Vec<_> = instance.enumerate_physical_devices()?;

    let mut graphics_qfi = None;
    let mut ace_qfi = None;
    let mut sdma_qfi = None;

    for device in &physical_devices {
        let physical_device_props = device.get_properties();
        let physical_device_memory_props = device.get_memory_properties();
        let queue_family_props: Vec<_> = device.get_queue_family_properties();

        let name = physical_device_props.get_device_name().to_string_lossy();
        let vram_heap_size = physical_device_memory_props.get_memory_heaps()
            .iter()
            .find_map(|x| x.flags.contains(vk::MemoryHeapFlags::DeviceLocal).then(|| x.size))
            .expect("No device local memory heap");

        println!("{}: {} MB VRAM", name, vram_heap_size / 1024 / 1024);

        graphics_qfi = queue_family_props.iter()
            .enumerate()
            .find_map(|(i, x)| x.queue_flags.contains(vk::QueueFlags::Graphics).then(|| i as u32));

        ace_qfi = queue_family_props.iter()
            .enumerate()
            .find_map(|(i, x)| (x.queue_flags.contains(vk::QueueFlags::Compute) && !x.queue_flags.contains(vk::QueueFlags::Graphics)).then(|| i as u32));

        sdma_qfi = queue_family_props.iter()
            .enumerate()
            .find_map(|(i, x)| (x.queue_flags.contains(vk::QueueFlags::Transfer) && !x.queue_flags.intersects(vk::QueueFlags::Graphics | vk::QueueFlags::Compute)).then(|| i as u32));

        println!("    Queue families: Graphics = {:?}, Compute/ACE = {:?}, Transfer/SDMA = {:?}", graphics_qfi, ace_qfi, sdma_qfi);
    }

    let physical_device = physical_devices.first().expect("No physical devices");
    let device_timestamp_unit = physical_device.get_properties().limits.timestamp_period;

    let mut all_queue_families = [graphics_qfi, ace_qfi, sdma_qfi].iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    all_queue_families.sort();
    all_queue_families.dedup();

    let queue_create_infos = all_queue_families.iter()
        .map(|i| {
            vk::DeviceQueueCreateInfo::default()
                .queue_family_index(*i)
                .queue_priorities(&[1.0])
        })
        .collect::<Vec<_>>();

    let device_create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_create_infos);

    let device = physical_device.create_device(&device_create_info)?;

    let queues = all_queue_families.iter()
        .map(|x| device.get_queue(*x, 0))
        .collect::<Vec<_>>();

    let query_pool_create_info = vk::QueryPoolCreateInfo::default()
        .query_type(vk::QueryType::Timestamp)
        .query_count(2);
    let query_pool = device.create_query_pool(&query_pool_create_info)?;
    let fence_create_info = vk::FenceCreateInfo::default()
        .flags(vk::FenceCreateFlags::Signaled);
    let fence = device.create_fence(&fence_create_info)?;

    for (queue, qfi) in queues.iter().zip(all_queue_families) {
        device.reset_fences(std::slice::from_ref(&fence))?;
        let command_pool_create_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(qfi)
            .flags(vk::CommandPoolCreateFlags::Transient);
        let command_pool = device.create_command_pool(&command_pool_create_info)?;

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(&command_pool)
            .level(vk::CommandBufferLevel::Primary)
            .command_buffer_count(1);

        let command_buffers: Vec<_> = device.allocate_command_buffers(&command_buffer_allocate_info)?;
        let command_buffer = command_buffers.first().expect("No command buffer");

        let command_buffer_begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::OneTimeSubmit);

        command_buffer.begin(&command_buffer_begin_info)?;

        command_buffer.reset_query_pool(&query_pool, 0, 2);
        command_buffer.write_timestamp(vk::PipelineStageFlags::TopOfPipe, &query_pool, 0);
        command_buffer.write_timestamp(vk::PipelineStageFlags::BottomOfPipe, &query_pool, 1);

        command_buffer.end()?;

        let submit_info = vk::SubmitInfo::default()
            .command_buffers(command_buffers.as_slice());

        queue.submit(&submit_info, Some(&fence))?;
        device.wait_for_fences(std::slice::from_ref(&fence), true, u64::MAX)?;

        let mut times: [u64; 2] = [0; 2];

        device.get_query_pool_results(&query_pool, 0, 2, 8, times.as_mut_ptr() as *mut c_void, 8, vk::QueryResultFlags::Result64 | vk::QueryResultFlags::Wait)?;

        let diff = (times[1] - times[0]) as f32 * device_timestamp_unit;
        println!("queue {} nop cb latency: {} ns", qfi, diff);

        unsafe {
            device.free_command_buffers(&command_pool, command_buffers.as_slice());
            device.destroy_command_pool(Some(&command_pool));
        }
    }

    unsafe {
        device.destroy_query_pool(Some(&query_pool));
        device.destroy_fence(Some(&fence));
        device.destroy();

        instance.destroy();
    }

    return Ok(());
}
