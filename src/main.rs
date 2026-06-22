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

    for device in physical_devices {
        let physical_device_props = device.get_properties();
        let physical_device_memory_props = device.get_memory_properties();

        let name = physical_device_props.get_device_name().to_string_lossy();
        let vram_heap_size = physical_device_memory_props.get_memory_heaps()
            .iter()
            .find_map(|x| { x.flags.contains(vk::MemoryHeapFlags::DeviceLocal).then(|| x.size) })
            .expect("No device local memory heap");

        println!("{}: {} MB VRAM", name, vram_heap_size / 1024 / 1024);
    }

    return Ok(());
}
