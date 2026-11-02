/// Separable box blur with clamped borders. Work is linear in pixel count,
/// including when the radius exceeds the image dimensions.
pub(super) fn blur(mask: &mut [f32], width: usize, height: usize, radius: usize) {
    assert_eq!(mask.len(), width * height);
    if radius == 0 || width == 0 || height == 0 { return; }
    let mut temporary = vec![0.0; mask.len()];
    blur_lines(mask, &mut temporary, width, height, 1, width, radius);
    blur_lines(&temporary, mask, height, width, width, 1, radius);
}

fn blur_lines(input: &[f32], output: &mut [f32], length: usize, count: usize,
    step: usize, line_step: usize, radius: usize) {
    let denominator = 2.0 * radius as f64 + 1.0;
    for line in 0..count {
        let base = line * line_step;
        // Accumulate in f64 to avoid drift across long rows/columns.
        let mut sum = input[base] as f64 * (radius as f64 + 1.0);
        for i in 1..=radius.min(length - 1) { sum += input[base + i * step] as f64; }
        sum += input[base + (length - 1) * step] as f64 * radius.saturating_sub(length - 1) as f64;
        for i in 0..length {
            output[base + i * step] = (sum / denominator) as f32;
            let leaving = i.saturating_sub(radius);
            let entering = i.saturating_add(radius).saturating_add(1).min(length - 1);
            sum += input[base + entering * step] as f64 - input[base + leaving * step] as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn direct(input: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
        let mut horizontal = vec![0.0; input.len()];
        let mut output = horizontal.clone();
        let r = radius as isize;
        for y in 0..h { for x in 0..w {
            let mut sum = 0.0f32;
            for dx in -r..=r { sum += input[y*w+(x as isize+dx).clamp(0,w as isize-1) as usize]; }
            horizontal[y*w+x] = sum / (2*radius+1) as f32;
        }}
        for y in 0..h { for x in 0..w {
            let mut sum = 0.0f32;
            for dy in -r..=r { sum += horizontal[(y as isize+dy).clamp(0,h as isize-1) as usize*w+x]; }
            output[y*w+x] = sum / (2*radius+1) as f32;
        }}
        output
    }

    #[test]
    fn preserves_clamped_box_filter_and_encoded_alpha() {
        for (w,h) in [(1,1),(1,17),(19,1),(9,7),(127,65)] {
            let input:Vec<f32>=(0..w*h).map(|i| ((i*73+19)%256) as f32/255.0).collect();
            for radius in [0,1,3,20,200] {
                let expected=direct(&input,w,h,radius);let mut actual=input.clone();blur(&mut actual,w,h,radius);
                for (a,b) in actual.iter().zip(expected) {
                    assert!((a-b).abs()<0.00001,"{w}x{h} radius{radius}: {a} vs {b}");
                    assert!(((*a*255.0) as u8).abs_diff((b*255.0) as u8)<=1);
                }
            }
        }
    }

    #[test]
    fn preserves_constant_and_handles_extreme_radius_without_large_kernel() {
        let mut mask=vec![0.375;35];blur(&mut mask,7,5,1_000_000);
        assert_eq!(mask,vec![0.375;35]);
        blur(&mut [],0,0,10);
    }

    #[test]
    fn centered_impulse_has_correct_support_and_mass() {
        let mut mask=vec![0.0;81];mask[4*9+4]=1.0;blur(&mut mask,9,9,2);
        for y in 0..9 {for x in 0..9 {
            assert!((mask[y*9+x]-if (2..=6).contains(&x)&&(2..=6).contains(&y) {0.04} else {0.0}).abs()<0.000001);
        }}
        assert!((mask.iter().sum::<f32>()-1.0).abs()<0.000001);
    }
}
