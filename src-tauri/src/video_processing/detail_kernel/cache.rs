use super::{DetailKernel, Plan};
use super::super::loupe::mip_pyramid::MipPyramid;
use anyhow::Result;
use std::collections::VecDeque;

struct Entry {
    revision: u64,
    region: [u32; 4],
    output: wgpu::Texture,
}

pub(crate) struct DetailCache {
    kernel: DetailKernel,
    source: Option<wgpu::Texture>,
    entries: VecDeque<Entry>,
    budget: u64,
    retained_bytes: u64,
}

pub(crate) struct ResolvedFrame {
    pub texture: wgpu::Texture,
    pub cache_hit: bool,
}

fn bytes(texture: &wgpu::Texture) -> u64 {
    mip_bytes([texture.width(), texture.height()])
}

fn mip_bytes(mut size: [u32; 2]) -> u64 {
    let mut total = 0;
    loop {
        total += size[0] as u64 * size[1] as u64 * 4;
        if size == [1, 1] { return total; }
        size = size.map(|v| (v / 2).max(1));
    }
}

impl DetailCache {
    pub fn new(kernel: DetailKernel, budget: u64) -> Self {
        Self { kernel, source: None, entries: VecDeque::new(), budget, retained_bytes: 0 }
    }

    pub fn retained_bytes(&self) -> u64 { self.retained_bytes }

    /// Submit source writes on this queue first, and change revision for every new frame.
    /// Returned textures remain valid after eviction; the budget covers cache-owned outputs.
    pub fn resolve(&mut self, device: &wgpu::Device, queue: &wgpu::Queue,
        source: &wgpu::Texture, revision: u64, plan: &Plan) -> Result<ResolvedFrame> {
        self.kernel.validate_region(device, source, plan)?;
        if self.source.as_ref() != Some(source) {
            self.entries.clear();
            self.retained_bytes = 0;
            self.source = Some(source.clone());
        }
        if let Some(index) = self.entries.iter().position(|entry| {
            entry.revision == revision && entry.region == plan.region
        }) {
            let entry = self.entries.remove(index).unwrap();
            let texture = entry.output.clone();
            self.entries.push_back(entry);
            return Ok(ResolvedFrame { texture, cache_hit: true });
        }
        let required = mip_bytes(plan.output_size);
        if required <= self.budget {
            while self.retained_bytes > self.budget - required {
                let entry = self.entries.pop_front().unwrap();
                self.retained_bytes -= bytes(&entry.output);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("detail frame") });
        let enhanced = self.kernel.encode(device, &mut encoder, source, plan)?;
        let pyramid = MipPyramid::new(device, &enhanced)?;
        pyramid.encode(&mut encoder);
        let texture = pyramid.texture().clone();
        // Publish only submitted work, so an abandoned caller encoder cannot create a false hit.
        queue.submit([encoder.finish()]);
        if required <= self.budget {
            self.entries.push_back(Entry { revision,
                region: plan.region, output: texture.clone() });
            self.retained_bytes += required;
        }
        Ok(ResolvedFrame { texture, cache_hit: false })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_all_mip_texels_for_odd_and_single_axis_outputs() {
        assert_eq!(mip_bytes([1, 1]), 4);
        assert_eq!(mip_bytes([9, 7]), (63 + 12 + 2 + 1) * 4);
        assert_eq!(mip_bytes([1, 17]), (17 + 8 + 4 + 2 + 1) * 4);
        assert_eq!(mip_bytes([110, 62]), (6820 + 1705 + 405 + 91 + 18 + 3 + 1) * 4);
    }
}
