use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct Tap {
    pub input: usize,
    pub offset: [i32; 2],
    pub matrix: [f32; 16],
    pub activation: i32,
    pub output: usize,
}
#[derive(Deserialize)]
pub(crate) struct Layer {
    pub bias: Vec<f32>,
    pub taps: Vec<Tap>,
}
#[derive(Deserialize)]
pub(crate) struct Model {
    pub scale: u32,
    pub layers: Vec<Layer>,
}
impl Model {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.scale, 2 | 4) || self.layers.len() != 8 {
            bail!("Unsupported detail model dimensions");
        }
        for (index, layer) in self.layers.iter().enumerate() {
            let channels = if index == 7 { self.scale * self.scale } else { 4 } as usize;
            if layer.bias.len() != channels || layer.taps.is_empty()
                || !layer.bias.iter().all(|v| v.is_finite()) {
                bail!("Invalid detail layer {index}");
            }
            for tap in &layer.taps {
                if tap.output >= channels / 4 || !(-1..=1).contains(&tap.activation)
                    || !tap.matrix.iter().all(|v| v.is_finite())
                    || (index < 7 && (tap.input != 0 || tap.offset.iter().any(|v| !(-1..=1).contains(v))))
                    || (index == 7 && (tap.input >= 7 || tap.offset != [0, 0])) {
                    bail!("Invalid detail layer {index} tap");
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(scale: u32) -> Model {
        Model { scale, layers: (0..8).map(|index| Layer {
            bias: vec![0.0; if index == 7 { (scale * scale) as usize } else { 4 }],
            taps: vec![Tap { input: 0, offset: [0, 0], matrix: [0.0; 16], activation: 0, output: 0 }],
        }).collect() }
    }
    #[test]
    fn validates_dimensions_and_accesses_before_shader_generation() {
        for scale in [2, 4] { model(scale).validate().unwrap(); }
        assert!(model(3).validate().is_err());
        let mut value = model(2);
        value.layers[0].taps[0].offset[0] = i32::MIN;
        assert!(value.validate().is_err());
        value = model(2);
        value.layers[7].taps[0].input = 7;
        assert!(value.validate().is_err());
        value = model(2);
        value.layers[7].taps[0].output = 1;
        assert!(value.validate().is_err());
        value = model(4);
        value.layers[2].bias[0] = f32::NAN;
        assert!(value.validate().is_err());
    }
}
