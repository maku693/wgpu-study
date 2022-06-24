pub struct CopyRenderer {
    render_pipeline: wgpu::RenderPipeline,
    bind_group0: wgpu::BindGroup,
    bind_group_layout1: wgpu::BindGroupLayout,
    bind_group1: Option<wgpu::BindGroup>,
}

impl CopyRenderer {
    pub fn new(device: &wgpu::Device, render_target_format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Copy Render Pass Bilinear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let vertex_shader_module =
            device.create_shader_module(&wgpu::include_wgsl!("vs_fullscreen.wgsl"));

        let bind_group_layout0 =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                }],
            });

        let bind_group_layout1 =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });

        let render_pipeline = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout0, &bind_group_layout1],
                push_constant_ranges: &[],
            });

            let fragment_shader_module =
                device.create_shader_module(&wgpu::include_wgsl!("fs_copy.wgsl"));

            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Post Process Sampling Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader_module,
                    entry_point: "main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader_module,
                    entry_point: "main",
                    targets: &[render_target_format.into()],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

        let bind_group0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout0,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            }],
        });

        Self {
            render_pipeline,
            bind_group0,
            bind_group_layout1,
            bind_group1: None,
        }
    }

    pub fn use_src_texture_view(
        &mut self,
        device: &wgpu::Device,
        src_texture_view: &wgpu::TextureView,
    ) {
        self.bind_group1 = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout1,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&src_texture_view),
            }],
        }));
    }

    pub fn draw<'rpass>(&'rpass self, rpass: &mut impl wgpu::util::RenderEncoder<'rpass>) {
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.bind_group0, &[]);
        rpass.set_bind_group(1, self.bind_group1.as_ref().unwrap(), &[]);
        rpass.draw(0..3, 0..1);
    }
}
