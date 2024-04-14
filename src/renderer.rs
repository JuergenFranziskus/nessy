use std::{num::NonZeroU64, sync::Arc};

use futures::executor::block_on;
use nessy::ppu::pixel_buffer::{PixelBuffer, PIXELS};
use wgpu::{
    include_wgsl, Adapter, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites,
    Device, DeviceDescriptor, Dx12Compiler, Face, FragmentState, FrontFace, Gles3MinorVersion,
    Instance, InstanceDescriptor, InstanceFlags, LoadOp, MultisampleState, Operations,
    PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PowerPreference, PresentMode,
    PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderStages, StoreOp,
    Surface, SurfaceConfiguration, TextureViewDescriptor, VertexState,
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

    pipeline: Pipeline,
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

        let renderer = Self {
            _instance: instance,
            _adapter: adapter,
            device,
            queue,
            surface,
            config,
            needs_reconfig: true,
            pipeline,
        };

        renderer.upload_palette();
        renderer
    }
    fn upload_palette(&self) {
        fn u8_to_f32(val: u8) -> f32 {
            (val as f32 / 255.0).clamp(0.0, 1.0)
        }

        let mut pped = Vec::with_capacity(64 * 4);
        for chunk in PALETTE.chunks_exact(3) {
            pped.push(u8_to_f32(chunk[0]));
            pped.push(u8_to_f32(chunk[1]));
            pped.push(u8_to_f32(chunk[2]));
            pped.push(1.0);
        }

        let as_bytes = bytemuck::cast_slice(&pped);
        self.queue
            .write_buffer(&self.pipeline.palette_buffer, 0, as_bytes);
    }

    pub fn window_event(&mut self, ev: &WindowEvent) {
        match ev {
            &WindowEvent::Resized(size) => self.resize(size),
            _ => (),
        }
    }
    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.config.width = size.width;
        self.config.height = size.height;
        self.needs_reconfig = true;
    }

    fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
        self.needs_reconfig = false;

        let size = [self.config.width, self.config.height];
        let bytes = bytemuck::cast_slice(&size);
        self.queue
            .write_buffer(&self.pipeline.screen_buffer, 0, bytes);
    }

    pub fn upload_pixels(&self, pixels: &PixelBuffer) {
        let bytes = bytemuck::cast_slice(&pixels.0);
        self.queue
            .write_buffer(&self.pipeline.pixel_buffer, 0, bytes);
    }

    pub fn render(&mut self) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        };
        if self.needs_reconfig {
            self.reconfigure_surface();
        }
        let Ok(tex) = self.surface.get_current_texture() else {
            return;
        };

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

            pass.set_pipeline(&self.pipeline.pipeline);
            pass.set_bind_group(0, &self.pipeline.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }

        self.queue.submit(Some(cmd.finish()));
        tex.present();
    }
}

fn create_render_pipeline(device: &Device, config: &SurfaceConfiguration) -> Pipeline {
    let src = include_wgsl!("shader.wgsl");
    let module = device.create_shader_module(src);

    let (bind_group_layout, pipeline_layout) = create_pipeline_layout(device);

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
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
    });

    let (pixel_buffer, screen_buffer, palette_buffer, bind_group) =
        create_bind_group(device, bind_group_layout);

    Pipeline {
        pipeline,
        pixel_buffer,
        screen_buffer,
        palette_buffer,
        bind_group,
    }
}
fn create_pipeline_layout(device: &Device) -> (BindGroupLayout, PipelineLayout) {
    let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(NonZeroU64::new(PIXELS as u64 * 4).unwrap()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(NonZeroU64::new(8).unwrap()),
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: Some(NonZeroU64::new(PALETTE_ENTRIES as u64 * 16).unwrap()),
                },
                count: None,
            },
        ],
    });

    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    (bind_group_layout, layout)
}
fn create_bind_group(
    device: &Device,
    layout: BindGroupLayout,
) -> (Buffer, Buffer, Buffer, BindGroup) {
    let pixel_buffer = device.create_buffer(&BufferDescriptor {
        label: None,
        size: PIXELS as u64 * 4,
        usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let screen_buffer = device.create_buffer(&BufferDescriptor {
        label: None,
        size: 8 as u64,
        usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
        mapped_at_creation: false,
    });
    let palette_buffer = device.create_buffer(&BufferDescriptor {
        label: None,
        size: PALETTE_ENTRIES as u64 * 16,
        usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: pixel_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: screen_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: palette_buffer.as_entire_binding(),
            },
        ],
    });

    (pixel_buffer, screen_buffer, palette_buffer, bind_group)
}

struct Pipeline {
    pipeline: RenderPipeline,
    pixel_buffer: Buffer,
    screen_buffer: Buffer,
    palette_buffer: Buffer,
    bind_group: BindGroup,
}

const PALETTE_ENTRIES: usize = 64;
static PALETTE: &[u8] = include_bytes!("ntscpalette.pal");
