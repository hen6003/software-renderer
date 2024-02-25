use std::{num::NonZeroU32, rc::Rc};

use glam::{UVec2, Vec2, Vec3, Vec3Swizzles};
use mesh_loader::Loader;
use palette::Srgb;
use softbuffer::{Buffer, SoftBufferError};
use winit::{
    error::EventLoopError,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

fn main() -> Result<(), EventLoopError> {
    env_logger::init();

    let loader = Loader::default();
    let scene = loader.load_obj("test.obj").unwrap();

    let event_loop = EventLoop::new().unwrap();
    let window = Rc::new(WindowBuilder::new().build(&event_loop).unwrap());
    let context = softbuffer::Context::new(window.clone()).unwrap();
    let mut surface = softbuffer::Surface::new(&context, window.clone()).unwrap();

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut frame = 0.0;
    let mut light_dir = Vec3::new(0.0, 0.0, -1.0);

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                log::info!("The close button was pressed; stopping");
                elwt.exit();
            }
            Event::AboutToWait => {
                // Application update code.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.

                //light_dir = Vec3::new(0.0, 1.0 * (frame as f32).sin(), 1.0 * (frame as f32).cos());

                //frame += 0.01;

                window.request_redraw();
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                let (width, height) = {
                    let size = window.inner_size();
                    (size.width, size.height)
                };

                surface
                    .resize(
                        NonZeroU32::new(width).unwrap(),
                        NonZeroU32::new(height).unwrap(),
                    )
                    .unwrap();

                let mut drawer = Drawer::new(surface.buffer_mut().unwrap(), width, height);
                drawer.clear();

                drawer.line((13.0, 20.0), (80.0, 40.0), Srgb::new(255, 255, 255));
                drawer.line((20.0, 13.0), (40.0, 80.0), Srgb::new(255, 0, 0));
                drawer.line((80.0, 40.0), (13.0, 20.0), Srgb::new(255, 0, 0));

                for mesh in &scene.meshes {
                    for face in &mesh.faces {
                        let world_coords: [Vec3; 3] = [
                            mesh.vertices[face[0] as usize].into(),
                            mesh.vertices[face[1] as usize].into(),
                            mesh.vertices[face[2] as usize].into(),
                        ];

                        let half_screen = drawer.screen_size.as_vec2() * Vec2::splat(0.5);
                        let screen_coords = [
                            ((world_coords[0].xy() + Vec2::ONE) * half_screen)
                                .extend(world_coords[0].z),
                            ((world_coords[1].xy() + Vec2::ONE) * half_screen)
                                .extend(world_coords[1].z),
                            ((world_coords[2].xy() + Vec2::ONE) * half_screen)
                                .extend(world_coords[2].z),
                        ];

                        let n = (world_coords[2] - world_coords[0])
                            .cross(world_coords[1] - world_coords[0])
                            .normalize();
                        let intensity = n.dot(light_dir);

                        if intensity > 0.0 {
                            let color: Srgb<u8> =
                                Srgb::new(intensity, intensity, intensity).into_format();

                            drawer.triangle(screen_coords, color);
                        }
                    }
                }

                drawer.finish().unwrap();
            }
            _ => (),
        }
    })
}

struct Drawer<'a> {
    buffer: Buffer<'a, Rc<Window>, Rc<Window>>,
    zbuffer: Vec<f32>,
    screen_size: UVec2,
}

impl<'a> Drawer<'a> {
    pub fn new(buffer: Buffer<'a, Rc<Window>, Rc<Window>>, width: u32, height: u32) -> Self {
        Self {
            buffer,
            zbuffer: vec![f32::NEG_INFINITY; (width * height) as usize],
            screen_size: (width, height).into(),
        }
    }

    pub fn finish(self) -> Result<(), SoftBufferError> {
        self.buffer.present()
    }

    pub fn screen_size(&self) -> UVec2 {
        self.screen_size
    }

    pub fn clear(&mut self) {
        for i in 0..(self.screen_size.x * self.screen_size.y) {
            self.buffer[i as usize] = 0;
        }
    }

    pub fn pixel<P, S, C>(&mut self, pos: P, color: C)
    where
        P: Into<UVec2>,
        C: Into<palette::rgb::Rgb<S, u8>>,
        S: std::fmt::Debug,
    {
        let pos = pos.into();
        let pos = UVec2::new(pos.x, self.screen_size.y - pos.y);
        let color = color.into();

        if (pos.y * self.screen_size.x + pos.x) < (self.screen_size.x * self.screen_size.y) as u32 {
            self.buffer[(pos.y * self.screen_size.x + pos.x) as usize] =
                color.blue as u32 | (color.green as u32) << 8 | (color.red as u32) << 16;
        }
    }

    pub fn line<P, S, C>(&mut self, pos_1: P, pos_2: P, color: C)
    where
        P: Into<Vec2>,
        C: Into<palette::rgb::Rgb<S, u8>> + Copy,
        S: std::fmt::Debug,
    {
        let mut pos_1 = pos_1.into();
        let mut pos_2 = pos_2.into();

        let steep = (pos_1.x - pos_2.x).abs() < (pos_1.y - pos_2.y).abs();

        if steep {
            // if the line is steep, we transpose the image
            std::mem::swap(&mut pos_1.x, &mut pos_1.y);
            std::mem::swap(&mut pos_2.x, &mut pos_2.y);
        }

        if pos_1.x > pos_2.x {
            // make it left−to−right
            std::mem::swap(&mut pos_1, &mut pos_2);
        }

        for t in (pos_1.x.round() as usize..pos_2.x.round() as usize)
            .map(|x| (x as f32 - pos_1.x) / (pos_2.x - pos_1.x))
        {
            let mut pos = pos_1 + (pos_2 - pos_1) * t;

            if steep {
                std::mem::swap(&mut pos.x, &mut pos.y);
            }

            self.pixel(pos.as_uvec2(), color);
        }
    }

    //fn barycentric(pts: [UVec2; 3], p: UVec2) -> Vec3 {
    //    let p = p.as_vec2();

    //    let p1 = pts[0].as_vec2();
    //    let p2 = pts[1].as_vec2();
    //    let p3 = pts[2].as_vec2();

    //    let lambda1 = ((p2.y - p3.y) * (p.x - p3.x) + (p3.x - p2.x) * (p.y - p3.y))
    //        / ((p2.y - p3.y) * (p1.x - p3.x) + (p3.x - p2.x) * (p1.y - p3.y));
    //    let lambda2 = ((p3.y - p1.y) * (p.x - p3.x) + (p1.x - p3.x) * (p.y - p3.y))
    //        / ((p2.y - p3.y) * (p1.x - p3.x) + (p3.x - p2.x) * (p1.y - p3.y));
    //    let lambda3 = 1.0 - lambda1 - lambda2;

    //    Vec3::new(lambda1, lambda2, lambda3)
    //}

    fn barycentric<P>(pts: [P; 3], p: Vec3) -> Vec3
    where
        P: Into<Vec3> + Copy,
    {
        let mut s = [Vec3::ZERO; 2];

        for i in (0..2).rev() {
            s[i][0] = pts[2].into()[i] - pts[0].into()[i];
            s[i][1] = pts[1].into()[i] - pts[0].into()[i];
            s[i][2] = pts[0].into()[i] - p[i];
        }

        let u = s[0].cross(s[1]);

        if u.z.abs() > 1e-2 {
            Vec3::new(1.0 - (u.x + u.y) / u.z, u.y / u.z, u.x / u.z)
        } else {
            -Vec3::ONE
        }
    }

    pub fn triangle<P, S, C>(&mut self, pts: [P; 3], color: C)
    where
        P: Into<Vec3> + Copy,
        C: Into<palette::rgb::Rgb<S, u8>> + Copy,
        S: std::fmt::Debug,
    {
        let mut bboxmin = self.screen_size().as_vec2();
        let mut bboxmax = Vec2::ZERO;
        let clamp = self.screen_size().as_vec2();

        for point in pts {
            bboxmin = Vec2::ZERO.max(bboxmin.min(point.into().truncate()));
            bboxmax = clamp.min(bboxmax.max(point.into().truncate()));
        }

        // Into integer coords
        let bboxmin = bboxmin.as_uvec2();
        let bboxmax = bboxmax.as_uvec2();

        for x in bboxmin.x..bboxmax.x {
            for y in bboxmin.y..bboxmax.y {
                let mut p = UVec2::new(x, y).as_vec2().extend(0.0);
                let bc_screen = Self::barycentric(pts, p);

                if bc_screen.x >= 0.0 && bc_screen.y >= 0.0 && bc_screen.z >= 0.0 {
                    for i in 0..3 {
                        p.z += pts[i].into().z * bc_screen[i];
                    }

                    let width = self.screen_size().x as f32;
                    if self.zbuffer[(p.x + p.y * width) as usize] < p.z {
                        self.zbuffer[(p.x + p.y * width) as usize] = p.z;

                        self.pixel(p.truncate().as_uvec2(), color)
                    }
                }
            }
        }
    }
}
