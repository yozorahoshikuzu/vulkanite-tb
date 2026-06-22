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

    return Ok(());
}
