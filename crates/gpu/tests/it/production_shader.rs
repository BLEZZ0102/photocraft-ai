//! Compile with production shader optimizations, on every hardware DX12 adapter.
//! Debug instances ask FXC to skip optimizations and can hide release-only failures.
use photocraft_color::{BlendMode, Color, ColorMode, SampleType};
use photocraft_doc::{Document, Size};
use photocraft_gpu::{Compositor, render_to_vec};

#[test]
fn optimized_fxc_composites_on_each_hardware_adapter() {
    let _lock = crate::GPU_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut options = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    options.backends = wgpu::Backends::DX12;
    options.flags = wgpu::InstanceFlags::empty();
    options.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
    let instance = wgpu::Instance::new(options);
    let mut tested = 0;
    for adapter in pollster::block_on(instance.enumerate_adapters(wgpu::Backends::DX12)) {
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            continue;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("hardware DX12 device");
        eprintln!("production FXC shaders: {}", info.name);
        for format in [wgpu::TextureFormat::Rgba16Float, wgpu::TextureFormat::Rgba32Float] {
            if !adapter
                .get_texture_format_features(format)
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
            {
                continue;
            }
            let mut comp = Compositor::try_new_with_format(&device, format).expect("production FXC compositor must compile");
            for sample in [SampleType::U8, SampleType::U16, SampleType::F32] {
                let mut doc = Document::with_background("synthetic", Size::new(2, 2), ColorMode::Rgb, sample, Color::rgba(0.2, 0.4, 0.6, 1.0));
                let mut top = doc.layers[0].clone();
                top.id = photocraft_doc::LayerId::fresh();
                top.opacity = 0.7;
                doc.layers.push(top);
                for blend in [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen, BlendMode::Overlay, BlendMode::ColorBurn, BlendMode::Divide] {
                    doc.layers[1].blend = blend;
                    let expected = photocraft_compose::flatten(&doc);
                    let actual = render_to_vec(&mut comp, &device, &queue, &doc, doc.bounds()).expect("production shader must render");
                    assert_eq!(actual.len(), expected.px.len(), "the entire synthetic image is returned");
                    for (cpu, gpu) in expected.px.iter().zip(actual.iter()) {
                        for (a, b) in cpu.iter().zip(gpu.iter()) {
                            assert!((a - b).abs() <= 1.0 / 255.0 + 4.0 * f32::EPSILON, "{} {format:?} {sample:?} {blend:?}: {a} vs {b}", info.name);
                        }
                    }
                }
            }
        }
        tested += 1;
    }
    if tested == 0 {
        assert!(std::env::var_os("WGPU_BACKEND").is_none(), "requested production FXC regression requires a hardware DX12 adapter");
        eprintln!("SKIP production FXC regression: no hardware DX12 adapter");
    }
}
