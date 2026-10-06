//! Platform graphics setup, independent of application state and agent execution.

pub fn configure(mut options: eframe::NativeOptions) -> anyhow::Result<eframe::NativeOptions> {
    // Be explicit: enabling eframe's wgpu feature changes its default renderer.
    options.renderer = eframe::Renderer::Glow;
    #[cfg(windows)]
    {
        let renderer = match std::env::var("CODECONVOY_RENDERER") {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => "auto".into(),
            Err(error) => return Err(error.into()),
        };
        windows::configure(&mut options, &renderer)?;
    }
    Ok(options)
}

pub fn startup_error(error: eframe::Error) -> anyhow::Error {
    #[cfg(windows)]
    let hint = "\n\nFor VM/display-driver problems, try software rendering in PowerShell:\n$env:CODECONVOY_RENDERER = 'software'; .\\CodeConvoy.exe\n\nSoftware rendering requires a working Windows Direct3D 12/WARP runtime.";
    #[cfg(not(windows))]
    let hint = "";
    anyhow::anyhow!("Unable to open CodeConvoy: {error}{hint}")
}

#[cfg(windows)]
mod windows {
    use eframe::{egui_wgpu, wgpu};

    pub(super) fn configure(
        options: &mut eframe::NativeOptions,
        renderer: &str,
    ) -> anyhow::Result<()> {
        match renderer {
            "opengl" => options.renderer = eframe::Renderer::Glow,
            "auto" | "software" => {
                options.renderer = eframe::Renderer::Wgpu;
                options.wgpu_options.wgpu_setup = setup(renderer == "software").into();
            }
            _ => anyhow::bail!(
                "Invalid CODECONVOY_RENDERER value {renderer:?}. Use auto, software, or opengl."
            ),
        }
        Ok(())
    }

    fn setup(software: bool) -> egui_wgpu::WgpuSetupCreateNew {
        let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
        // Auto mode prefers a compatible GPU but also considers the DX12 WARP
        // adapter. No Vulkan/OpenGL driver is needed in a Windows guest.
        setup.instance_descriptor.backends = wgpu::Backends::DX12;
        if software {
            setup.native_adapter_selector = Some(std::sync::Arc::new(|adapters, surface| {
                adapters.iter().find(|adapter| {
                    adapter.get_info().device_type == wgpu::DeviceType::Cpu
                        && surface.is_none_or(|surface| adapter.is_surface_supported(surface))
                }).cloned().ok_or_else(|| {
                    "No compatible Direct3D 12 software adapter (WARP) is available. Update Windows, or use CODECONVOY_RENDERER=opengl with an OpenGL-capable display driver.".into()
                })
            }));
        }
        setup
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn renderer_choices_require_dx12_and_software_never_uses_a_hardware_adapter() {
            let mut options = eframe::NativeOptions::default();
            super::configure(&mut options, "auto").expect("auto config");
            assert_eq!(options.renderer, eframe::Renderer::Wgpu);
            let super::egui_wgpu::WgpuSetup::CreateNew(setup) = &options.wgpu_options.wgpu_setup
            else {
                panic!("new setup expected");
            };
            assert_eq!(
                setup.instance_descriptor.backends,
                super::wgpu::Backends::DX12
            );
            assert!(setup.native_adapter_selector.is_none());
            super::configure(&mut options, "software").expect("software config");
            let super::egui_wgpu::WgpuSetup::CreateNew(setup) = &options.wgpu_options.wgpu_setup
            else {
                panic!("new setup expected");
            };
            assert_eq!(
                setup.instance_descriptor.backends,
                super::wgpu::Backends::DX12
            );
            assert!(
                setup
                    .native_adapter_selector
                    .as_ref()
                    .expect("CPU selector")(&[], None)
                .expect_err("no silent hardware fallback")
                .contains("WARP")
            );
            super::configure(&mut options, "opengl").expect("OpenGL config");
            assert_eq!(options.renderer, eframe::Renderer::Glow);
            assert!(super::configure(&mut options, "typo").is_err());
        }

        #[test]
        #[ignore = "Requires Windows Direct3D 12/WARP; explicitly exercised by Windows CI"]
        fn software_adapter_initializes_egui_renderer() {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime");
            runtime.block_on(async {
                let config = super::egui_wgpu::WgpuConfiguration {
                    wgpu_setup: super::setup(true).into(),
                    ..Default::default()
                };
                let instance = config.wgpu_setup.new_instance().await;
                // Same selection, device requirements, and shader/pipeline setup
                // as the app; this headless check does not verify presentation.
                let render = super::egui_wgpu::RenderState::create(
                    &config,
                    &instance,
                    None,
                    Default::default(),
                )
                .await
                .expect("WARP must initialize egui without OpenGL or a hardware GPU");
                let info = render.adapter.get_info();
                assert_eq!(info.backend, super::wgpu::Backend::Dx12);
                assert_eq!(info.device_type, super::wgpu::DeviceType::Cpu);
                println!("Software renderer: {} ({:?})", info.name, info.backend);
            });
        }
    }
}
