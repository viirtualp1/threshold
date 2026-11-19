use std::error::Error;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use wgpu::*;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::hud::{self, Text};
use crate::scene::{PORTALS, PROPS, ROOMS};
use crate::sim::Simulation;

const SCENE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;
const PIXEL_BUDGET: f32 = 830_000.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuRoom {
    min: [f32; 4],
    max: [f32; 4],
    ranges: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuDoor {
    bottom: [f32; 4],
    inward: [f32; 4],
    axis: [f32; 4],
    link: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuProp {
    min: [f32; 4],
    max: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuScene {
    rooms: [GpuRoom; ROOMS.len()],
    doors: [GpuDoor; PORTALS.len()],
    props: [GpuProp; PROPS.len()],
    glyphs: [[u32; 2]; 64],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuBall {
    center: [f32; 4],
    room: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Frame {
    eye: [f32; 4],
    forward: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    display: [f32; 4],
    flags: [u32; 4],
    balls: [GpuBall; 7],
    text: [Text; 10],
}

struct Targets {
    size: (u32, u32),
    view: TextureView,
    present: BindGroup,
}

pub struct Graphics {
    window: Arc<Window>,
    instance: Instance,
    surface: Surface<'static>,
    configuration: SurfaceConfiguration,
    device: Device,
    queue: Queue,
    frame: Buffer,
    scene: Buffer,
    sampler: Sampler,
    room_pipeline: RenderPipeline,
    present_pipeline: RenderPipeline,
    room_bindings: BindGroup,
    targets: Targets,
    drawable: bool,
}

impl Graphics {
    pub async fn new(window: Arc<Window>) -> Result<Self, Box<dyn Error>> {
        let instance = Instance::new(InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        let (device, queue) = adapter.request_device(&DeviceDescriptor::default()).await?;
        let size = window.inner_size();
        let mut configuration = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("the GPU cannot present to this window")?;
        let capabilities = surface.get_capabilities(&adapter);
        if let Some(format) = capabilities
            .formats
            .into_iter()
            .find(TextureFormat::is_srgb)
        {
            configuration.format = format;
        }
        surface.configure(&device, &configuration);

        let shader = device.create_shader_module(include_wgsl!("room.wgsl"));
        let room_pipeline = pipeline(&device, &shader, "room_fragment", SCENE_FORMAT);
        let present_pipeline = pipeline(&device, &shader, "present_fragment", configuration.format);
        let frame = device.create_buffer(&BufferDescriptor {
            label: Some("Room camera and ball"),
            size: size_of::<Frame>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Shared room graph"),
            contents: bytemuck::bytes_of(&GpuScene::new()),
            usage: BufferUsages::STORAGE,
        });
        let sampler = device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let room_bindings = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Room bindings"),
            layout: &room_pipeline.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: scene.as_entire_binding(),
                },
            ],
        });
        let targets = Targets::new(
            &device,
            &present_pipeline,
            &frame,
            &scene,
            &sampler,
            render_size(size),
        );

        Ok(Self {
            window,
            instance,
            surface,
            configuration,
            device,
            queue,
            frame,
            scene,
            sampler,
            room_pipeline,
            present_pipeline,
            room_bindings,
            targets,
            drawable: size.width > 0 && size.height > 0,
        })
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.drawable = size.width > 0 && size.height > 0;
        if !self.drawable {
            return;
        }

        self.configuration.width = size.width;
        self.configuration.height = size.height;
        self.surface.configure(&self.device, &self.configuration);
        let scaled = render_size(size);

        if self.targets.size != scaled {
            self.targets = Targets::new(
                &self.device,
                &self.present_pipeline,
                &self.frame,
                &self.scene,
                &self.sampler,
                scaled,
            );
        }
    }

    pub fn draw(&mut self, sim: &Simulation, captured: bool) -> Result<bool, Box<dyn Error>> {
        if !self.drawable {
            return Ok(false);
        }

        let (output, reconfigure) = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture) => (texture, false),
            CurrentSurfaceTexture::Suboptimal(texture) => (texture, true),
            CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return Ok(false),
            CurrentSurfaceTexture::Outdated => {
                self.resize(self.window.inner_size());
                return Ok(false);
            }
            CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(self.window.clone())?;
                self.resize(self.window.inner_size());
                return Ok(false);
            }
            CurrentSurfaceTexture::Validation => {
                return Err("the GPU could not acquire a valid frame".into());
            }
        };

        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());
        let frame = Frame::new(sim, &self.configuration, captured);
        self.queue
            .write_buffer(&self.frame, 0, bytemuck::bytes_of(&frame));

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Threshold frame"),
            });
        pass(
            &mut encoder,
            &self.targets.view,
            &self.room_pipeline,
            &self.room_bindings,
        );
        pass(
            &mut encoder,
            &view,
            &self.present_pipeline,
            &self.targets.present,
        );
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(output);

        if reconfigure {
            self.resize(self.window.inner_size());
        }
        Ok(true)
    }

    pub fn wait_for_frame(&self) -> Result<(), PollError> {
        self.device.poll(PollType::wait_indefinitely())?;
        Ok(())
    }
}

impl GpuScene {
    fn new() -> Self {
        Self {
            rooms: std::array::from_fn(|index| {
                let room = &ROOMS[index];
                GpuRoom {
                    min: room.min().with(0.0),
                    max: room.max().with(0.0),
                    ranges: [
                        room.portals.start as u32,
                        room.portals.end as u32,
                        room.props.start as u32,
                        room.props.end as u32,
                    ],
                }
            }),
            doors: PORTALS.map(|door| GpuDoor {
                bottom: door.bottom.with(door.span[0]),
                inward: door.inward.with(door.span[1]),
                axis: door.right().with(0.0),
                link: [door.link as u32, door.room as u32, door.style as u32, 0],
            }),
            props: PROPS.map(|prop| GpuProp {
                min: prop.min.with(prop.material as f32),
                max: prop.max.with(0.0),
            }),
            glyphs: hud::font(),
        }
    }
}

impl Frame {
    fn new(sim: &Simulation, config: &SurfaceConfiguration, captured: bool) -> Self {
        let player = sim.player;
        let ball = sim.ball;
        let mut balls = [GpuBall::zeroed(); 7];
        balls[0] = GpuBall {
            center: ball.center.with(ball.radius()),
            room: [ball.room as u32, 0, 0, 0],
        };
        let mut count = 1;

        // A sphere crossing a threshold has one clipped image in each of its two rooms.
        for index in ROOMS[ball.room].portals.clone() {
            let door = PORTALS[index];
            let depth = (ball.center - door.bottom).dot(door.inward);
            if depth < ball.radius()
                && door.fits(ball.center, ball.radius(), ball.radius(), ball.radius())
            {
                balls[count] = GpuBall {
                    center: door.point(ball.center).with(ball.radius() * door.ratio()),
                    room: [door.destination().room as u32, 0, 0, 0],
                };
                count += 1;
            }
        }

        let (state, action) = if ball.held {
            (2, "E / CLICK  THROW BALL")
        } else if sim.can_grab() {
            (1, "E / CLICK  GRAB BALL")
        } else {
            (0, "CLICK TO CAPTURE MOUSE")
        };

        let text = [
            "THRESHOLD",
            "IMPOSSIBLE APARTMENT",
            ROOMS[player.room].name,
            hud::scale_label(player.size),
            "WASD MOVE  MOUSE LOOK  SHIFT SPRINT",
            "E INTERACT  R RESET  F FULLSCREEN  ESC RELEASE",
            action,
            "01 / DIRECT",
            "02 / 1:10",
            "03 / GALLERY",
        ]
        .map(hud::pack);

        Self {
            eye: player.head().with(0.0),
            forward: player.forward().with(0.0),
            right: player.right().with(0.0),
            up: player.up().with(0.0),
            display: [
                config.width as f32,
                config.height as f32,
                0.0,
                if config.format.is_srgb() { 1.0 } else { 0.0 },
            ],
            flags: [player.room as u32, count as u32, state, u32::from(captured)],
            balls,
            text,
        }
    }
}

impl Targets {
    fn new(
        device: &Device,
        pipeline: &RenderPipeline,
        frame: &Buffer,
        scene: &Buffer,
        sampler: &Sampler,
        size: (u32, u32),
    ) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Pixel-budgeted room view"),
            size: Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: SCENE_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        let present = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Screen and HUD bindings"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: scene.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Sampler(sampler),
                },
            ],
        });
        Self {
            size,
            view,
            present,
        }
    }
}

fn render_size(available: PhysicalSize<u32>) -> (u32, u32) {
    let width = available.width.max(1) as f32;
    let height = available.height.max(1) as f32;
    let proportion = (PIXEL_BUDGET / (width * height)).sqrt().min(1.0);
    (
        (width * proportion).floor().max(1.0) as u32,
        (height * proportion).floor().max(1.0) as u32,
    )
}

fn pipeline(
    device: &Device,
    shader: &ShaderModule,
    fragment: &str,
    format: TextureFormat,
) -> RenderPipeline {
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(fragment),
        layout: None,
        vertex: VertexState {
            module: shader,
            entry_point: Some("vertex"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(ColorTargetState {
                format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn pass(
    encoder: &mut CommandEncoder,
    output: &TextureView,
    pipeline: &RenderPipeline,
    bindings: &BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(RenderPassColorAttachment {
            view: output,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color::BLACK),
                store: StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bindings, &[]);
    pass.draw(0..3, 0..1);
}
