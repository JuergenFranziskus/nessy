use std::sync::Arc;

use futures::executor::block_on;
use wgpu::{
    include_wgsl, Adapter, Backends, Color, ColorTargetState, ColorWrites, Device,
    DeviceDescriptor, Dx12Compiler, Face, FragmentState, FrontFace, Gles3MinorVersion, Instance,
    InstanceDescriptor, InstanceFlags, LoadOp, MultisampleState, Operations, PolygonMode,
    PowerPreference, PresentMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    RequestAdapterOptions, StoreOp, Surface, SurfaceConfiguration, TextureViewDescriptor,
    VertexState,
};
use winit::{dpi::PhysicalSize, event::WindowEvent, window::Window};

pub struct Renderer {
    _instance: Instance,
    _adapter: Adapter,
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    needs_reconfig: bool,
    force_reconfig: bool,

    pipeline: RenderPipeline,
}
impl Renderer {
    pub fn init(window: Arc<Window>) -> Self {
        let backends = Backends::VULKAN;
        let size = window.inner_size();

        let instance = Instance::new(InstanceDescriptor {
            backends,
            flags: InstanceFlags::empty(),
            dx12_shader_compiler: Dx12Compiler::Fxc,
            gles_minor_version: Gles3MinorVersion::Automatic,
        });

        let surface = instance.create_surface(window).unwrap();
        let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .unwrap();

        let mut config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        config.present_mode = PresentMode::Fifo;

        let (device, queue) = block_on(adapter.request_device(
            &DeviceDescriptor {
                label: None,
                required_features: adapter.features(),
                required_limits: adapter.limits(),
            },
            None,
        ))
        .unwrap();

        let pipeline = create_render_pipeline(&device, &config);

        Self {
            _instance: instance,
            _adapter: adapter,
            device,
            queue,
            surface,
            config,
            needs_reconfig: true,
            force_reconfig: false,
            pipeline,
        }
    }

    pub fn window_event(&mut self, ev: &WindowEvent) {
        match ev {
            &WindowEvent::Focused(focus) => self.force_reconfig = !focus,
            &WindowEvent::Resized(size) => self.resize(size),
            _ => (),
        }
    }
    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.config.width = size.width;
        self.config.height = size.height;
        self.needs_reconfig = true;
    }

    fn acquire(&mut self) -> wgpu::SurfaceTexture {
        let surface = &mut self.surface;

        match surface.get_current_texture() {
            Ok(frame) => frame,
            // If we timed out, just try again
            Err(wgpu::SurfaceError::Timeout) => surface
                .get_current_texture()
                .expect("Failed to acquire next surface texture!"),
            Err(
                // If the surface is outdated, or was lost, reconfigure it.
                wgpu::SurfaceError::Outdated
                | wgpu::SurfaceError::Lost
                // If OutOfMemory happens, reconfiguring may not help, but we might as well try
                | wgpu::SurfaceError::OutOfMemory,
            ) => {
                surface.configure(&self.device, &self.config);
                surface
                    .get_current_texture()
                    .expect("Failed to acquire next surface texture!")
            }
        }
    }
    pub fn render(&mut self) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        };
        if self.needs_reconfig || self.force_reconfig {
            self.surface.configure(&self.device, &self.config);
            self.needs_reconfig = false;
        }
        let tex = self.acquire();

        let mut cmd = self.device.create_command_encoder(&Default::default());
        {
            let view = tex.texture.create_view(&TextureViewDescriptor::default());
            let mut pass = cmd.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::GREEN),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.draw(0..6, 0..1);
        }

        self.queue.submit(Some(cmd.finish()));
        tex.present();
    }
}

fn create_render_pipeline(device: &Device, config: &SurfaceConfiguration) -> RenderPipeline {
    let src = include_wgsl!("shader.wgsl");
    let module = device.create_shader_module(src);

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: VertexState {
            module: &module,
            entry_point: "vs_main",
            buffers: &[],
        },
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Cw,
            cull_mode: Some(Face::Back),
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(FragmentState {
            module: &module,
            entry_point: "fs_main",
            targets: &[Some(ColorTargetState {
                format: config.format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview: None,
    })
}
