//! Validated, bounded draw packets for the native D3D11 compositor. Geometry is
//! CPU-generated; blending, scaling and green-key evaluation happen on the GPU.
use crate::{
    layers::{Layer, Raster},
    overlay::{Annotation, Shape},
    webcam::{Image, Placement},
    Error, Result,
};
use std::sync::Arc;

#[repr(C)]
pub(crate) struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
pub(crate) struct Draw {
    mode: u32,
    slot: u32,
    first: u32,
    count: u32,
    parameters: [f32; 4],
}
#[repr(C)]
pub(crate) struct Texture {
    pixels: *const u8,
    identity: u64,
    width: u32,
    height: u32,
    bgra: u32,
    slot: u32,
}

pub(crate) struct Packet<'a> {
    pub textures: Vec<Texture>,
    pub vertices: Vec<Vertex>,
    pub draws: Vec<Draw>,
    pub retained: Vec<Arc<Raster>>,
    // Camera bytes are borrowed only until the synchronous GPU upload returns.
    _camera: Option<&'a Image>,
    size: [f32; 2],
}
impl<'a> Packet<'a> {
    pub fn new(
        size: [u32; 2],
        layers: &[Layer],
        marks: &[Annotation],
        cursor: &[Annotation],
        camera: Option<(&'a Image, &Placement)>,
        camera_id: u64,
    ) -> Result<Self> {
        if size.iter().any(|&v| !(2..=7680).contains(&v))
            || layers.len() > 5
            || marks.len() > 64
            || cursor.len() > 2
        {
            return Err(Error::Invalid("GPU composition budget exceeded"));
        }
        let mut p = Self {
            textures: Vec::new(),
            vertices: Vec::new(),
            draws: Vec::new(),
            retained: Vec::new(),
            _camera: camera.map(|v| v.0),
            size: size.map(|v| v as f32),
        };
        if let Some((image, place)) = camera {
            if !place.valid()
                || image.width == 0
                || image.height == 0
                || image.width > 640
                || image.height > 480
                || image.bgra.len() != image.width * image.height * 4
            {
                return Err(Error::Invalid("invalid GPU camera image"));
            }
            p.textures.push(Texture {
                pixels: image.bgra.as_ptr(),
                identity: camera_id,
                width: image.width as u32,
                height: image.height as u32,
                bgra: 1,
                slot: 5,
            });
            p.image(
                5,
                [image.width as f32, image.height as f32],
                [place.x, place.y, place.width],
                [1.0, place.similarity, place.softness, 0.0],
                if place.key { 3 } else { 0 },
                place.mirror,
            );
        }
        p.annotations(marks)?;
        for (slot, layer) in layers.iter().enumerate() {
            if !layer.valid() {
                return Err(Error::Invalid("invalid GPU raster placement"));
            }
            let raster = &layer.raster;
            p.textures.push(Texture {
                pixels: raster.gpu_rgba().as_ptr(),
                identity: Arc::as_ptr(raster) as usize as u64,
                width: raster.width as u32,
                height: raster.height as u32,
                bgra: 0,
                slot: slot as u32,
            });
            p.retained.push(raster.clone());
            p.image(
                slot as u32,
                [raster.width as f32, raster.height as f32],
                [layer.x, layer.y, layer.width],
                [layer.opacity, 0.0, 0.0, 0.0],
                0,
                false,
            );
        }
        p.annotations(cursor)?;
        Ok(p)
    }
    pub fn fade(&mut self, alpha: f32) -> Result<()> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(Error::Invalid("invalid scene fade"));
        }
        if alpha == 0.0 {
            return Ok(());
        }
        let first = self.vertices.len() as u32;
        for [u, v] in [[0., 0.], [1., 0.], [0., 1.], [0., 1.], [1., 0.], [1., 1.]] {
            self.vertices.push(Vertex {
                position: [u * self.size[0], v * self.size[1]],
                uv: [0.; 2],
                color: [0., 0., 0., alpha],
            });
        }
        self.draws.push(Draw {
            mode: 1,
            slot: 0,
            first,
            count: 6,
            parameters: [0.; 4],
        });
        Ok(())
    }
    fn image(
        &mut self,
        slot: u32,
        source: [f32; 2],
        placement: [f32; 3],
        parameters: [f32; 4],
        mode: u32,
        mirror: bool,
    ) {
        let [fw, fh] = self.size;
        let scale = (fw * placement[2] / source[0]).min(fh / source[1]);
        let w = (source[0] * scale).round().clamp(1.0, fw);
        let h = (source[1] * scale).round().clamp(1.0, fh);
        let x = (((fw - w) * placement[0]).round() as u32 & !1) as f32;
        let y = (((fh - h) * placement[1]).round() as u32 & !1) as f32;
        let first = self.vertices.len();
        for [u, v] in [[0., 0.], [1., 0.], [0., 1.], [0., 1.], [1., 0.], [1., 1.]] {
            self.vertices.push(Vertex {
                position: [x + u * w, y + v * h],
                // Pixel centers normally sample (x + .5) / output_width.
                // Offset to match the CPU nearest-neighbor floor mapping, so
                // cached Unicode glyphs retain the same source-pixel coverage.
                uv: [
                    if mirror {
                        1.0 - (u - 0.5 / w + 0.000001)
                    } else {
                        u - 0.5 / w + 0.000001
                    },
                    v - 0.5 / h + 0.000001,
                ],
                color: [1.; 4],
            });
        }
        self.draws.push(Draw {
            mode,
            slot,
            first: first as u32,
            count: 6,
            parameters,
        });
    }
    fn line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let length = dx.hypot(dy);
        let (nx, ny) = if length > 0.0 {
            (-dy / length * width / 2., dx / length * width / 2.)
        } else {
            (width / 2., 0.)
        };
        let b = if length > 0.0 {
            b
        } else {
            [b[0], b[1] + width]
        };
        let corners = [
            [a[0] + nx, a[1] + ny],
            [b[0] + nx, b[1] + ny],
            [a[0] - nx, a[1] - ny],
            [b[0] - nx, b[1] - ny],
        ];
        for i in [0, 1, 2, 2, 1, 3] {
            self.vertices.push(Vertex {
                position: corners[i],
                uv: [0.; 2],
                color,
            });
        }
    }
    fn annotations(&mut self, marks: &[Annotation]) -> Result<()> {
        for mark in marks {
            if mark.points.is_empty()
                || mark.points.len() > 256
                || !(1..=16).contains(&mark.width)
                || mark
                    .points
                    .iter()
                    .flatten()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            {
                return Err(Error::Invalid("invalid GPU annotation geometry"));
            }
            let points: Vec<_> = mark
                .points
                .iter()
                .map(|p| [p[0] * (self.size[0] - 1.), p[1] * (self.size[1] - 1.)])
                .collect();
            let color = [
                mark.rgb[0] as f32 / 255.,
                mark.rgb[1] as f32 / 255.,
                mark.rgb[2] as f32 / 255.,
                1.,
            ];
            let first = self.vertices.len();
            let mut mode = 1;
            let mut parameters = [0.; 4];
            match mark.shape {
                Shape::Spotlight => {
                    mode = 2;
                    parameters = [
                        points[0][0],
                        points[0][1],
                        self.size[0].min(self.size[1]) / 7.,
                        0.,
                    ];
                    for [u, v] in [[0., 0.], [1., 0.], [0., 1.], [0., 1.], [1., 0.], [1., 1.]] {
                        self.vertices.push(Vertex {
                            position: [u * self.size[0], v * self.size[1]],
                            uv: [0.; 2],
                            color,
                        });
                    }
                }
                Shape::Box if points.len() >= 2 => {
                    let a = points[0];
                    let b = *points.last().unwrap();
                    for (a, b) in [
                        (a, [b[0], a[1]]),
                        ([b[0], a[1]], b),
                        (b, [a[0], b[1]]),
                        ([a[0], b[1]], a),
                    ] {
                        self.line(a, b, mark.width as f32, color);
                    }
                }
                Shape::Arrow if points.len() >= 2 => {
                    let a = points[0];
                    let b = *points.last().unwrap();
                    self.line(a, b, mark.width as f32, color);
                    let angle = (b[1] - a[1]).atan2(b[0] - a[0]);
                    let length = (self.size[0].min(self.size[1]) * 0.04).clamp(8., 60.);
                    for delta in [-0.5, 0.5] {
                        self.line(
                            b,
                            [
                                b[0] - length * (angle + delta).cos(),
                                b[1] - length * (angle + delta).sin(),
                            ],
                            mark.width as f32,
                            color,
                        );
                    }
                }
                _ => {
                    if points.len() == 1 {
                        self.line(points[0], points[0], mark.width as f32, color);
                    }
                    for pair in points.windows(2) {
                        self.line(pair[0], pair[1], mark.width as f32, color);
                    }
                }
            }
            self.draws.push(Draw {
                mode,
                slot: 0,
                first: first as u32,
                count: (self.vertices.len() - first) as u32,
                parameters,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_packets_and_bounds_worst_case_geometry() {
        let mark = Annotation {
            shape: Shape::Stroke,
            points: vec![[0.5, 0.5]; 256],
            rgb: [255, 0, 0],
            width: 16,
        };
        let marks = vec![mark.clone(); 64];
        let packet = Packet::new([3840, 2160], &[], &marks, &[], None, 0).unwrap();
        assert_eq!(packet.vertices.len(), 64 * 255 * 6);
        assert!(packet.vertices.len() < 110000);
        assert!(Packet::new([640, 360], &[], &vec![mark; 65], &[], None, 0).is_err());
        let bad = Annotation {
            shape: Shape::Box,
            points: vec![[f32::NAN, 0.]],
            rgb: [0; 3],
            width: 1,
        };
        assert!(Packet::new([640, 360], &[], &[bad], &[], None, 0).is_err());
    }
    #[test]
    fn raster_identity_is_retained_and_order_places_cursor_last() {
        let raster = Arc::new(Raster::rgba(2, 2, &[255; 16]).unwrap());
        let layer = Layer {
            raster: raster.clone(),
            x: 1.,
            y: 1.,
            width: 0.2,
            opacity: 0.5,
        };
        let mark = Annotation {
            shape: Shape::Spotlight,
            points: vec![[0.5, 0.5]],
            rgb: [0; 3],
            width: 1,
        };
        let packet = Packet::new(
            [640, 360],
            &[layer],
            std::slice::from_ref(&mark),
            std::slice::from_ref(&mark),
            None,
            0,
        )
        .unwrap();
        assert_eq!(
            packet.draws.iter().map(|d| d.mode).collect::<Vec<_>>(),
            [2, 0, 2]
        );
        assert_eq!(packet.draws[1].parameters[0], 0.5);
        assert!(Arc::ptr_eq(&raster, &packet.retained[0]));
        assert_eq!(std::mem::size_of::<Vertex>(), 32);
        assert_eq!(std::mem::size_of::<Draw>(), 32);
        assert_eq!(std::mem::size_of::<Texture>(), 32);
    }
}
