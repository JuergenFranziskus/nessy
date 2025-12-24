use std::sync::Arc;

use bytemuck::cast_slice;
use smol::block_on;
use wgpu::*;
use winit::window::Window;

pub struct Render {
    instance: Instance,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
    config: SurfaceConfiguration,

    texture: Texture,
    sampler: Sampler,
    bind_group: BindGroup,
    pipeline: RenderPipeline,
}
impl Render {
    pub fn new(window: Arc<Window>) -> Self {
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::VULKAN,
            flags: InstanceFlags::from_env_or_default(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::from_env_or_default(),
        });
        let size = window.inner_size();
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        });
        let adapter = block_on(adapter).unwrap();

        let features = Features::default();
        let limits = Limits::defaults();
        let device = adapter.request_device(&DeviceDescriptor {
            label: None,
            required_features: features,
            required_limits: limits,
            experimental_features: ExperimentalFeatures::disabled(),
            memory_hints: MemoryHints::Performance,
            trace: Trace::Off,
        });
        let (device, queue) = block_on(device).unwrap();

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();

        surface.configure(&device, &config);

        let texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: 256,
                height: 240,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: None,
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let shader = include_wgsl!("shader.wgsl");
        let shader = device.create_shader_module(shader);

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Cw,
                cull_mode: None,
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
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            instance,
            adapter,
            device,
            queue,
            surface,
            config,
            texture,
            sampler,
            bind_group,
            pipeline,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&mut self, framebuffer: &[u32; 256 * 240]) {
        let texture = self.surface.get_current_texture().unwrap();
        let view = texture.texture.create_view(&Default::default());

        self.queue.write_texture(
            self.texture.as_image_copy(),
            cast_slice(framebuffer),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4),
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 240,
                depth_or_array_layers: 1,
            },
        );

        let mut cmd = self.device.create_command_encoder(&Default::default());

        let mut pass = cmd.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..6, 0..1);

        drop(pass);
        self.queue.submit([cmd.finish()]);
        texture.present();
    }
}
