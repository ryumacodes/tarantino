pub(super) fn level(source:[f32;2],before:[f32;4],now:[f32;4],output:[f32;2]) -> u32 {
    let mut footprint=(source[0]/before[2].min(now[2])).max(source[1]/before[3].min(now[3]));
    if before!=now {
        let mut travel=0.0_f32;
        for x in [0.0,output[0]] {for y in [0.0,output[1]] {
            let dx=((x-before[0])/before[2]-(x-now[0])/now[2])*source[0];
            let dy=((y-before[1])/before[3]-(y-now[1])/now[3])*source[1];
            travel=travel.max(dx.hypot(dy));
        }}
        // Uncapped taps span at most four pixels; capped taps divide travel by sixteen.
        footprint=footprint.max(4.0*(source[0]/before[2]).max(source[1]/before[3])).max(travel/16.0);
    }
    footprint.log2().max(0.0).floor() as u32+1
}

#[cfg(test)]
mod tests {
    use super::level;
    #[test]
    fn shutter_levels_keep_static_sampling_and_bound_long_motion() {
        let source=[512.0;2];let before=[127.5,127.5,512.0,512.0];let now=[-0.5,127.5,512.0,512.0];
        assert_eq!(level(source,before,before,[128.0;2]),1);
        assert_eq!(level(source,before,now,[128.0;2]),4);
    }
    #[test]
    fn clipped_segments_fit_the_shutter_mip_bound() {
        let source=[1919.0,1079.0];
        for zoom in [0.25,1.0,2.0,8.0] {
            let before=[355.2,214.7,800.0*zoom,450.0*zoom];
            let now=[735.1,-25.2,900.0/zoom,600.0/zoom];
            let bound=level(source,before,now,[960.0,540.0]);
            let texels=(source[0]/before[2]).max(source[1]/before[3]);
            for x in (0..=960).step_by(60) {for y in (0..=540).step_by(45) {
                let dx=((x as f32-before[0])/before[2]-(x as f32-now[0])/now[2])*source[0];
                let dy=((y as f32-before[1])/before[3]-(y as f32-now[1])/now[3])*source[1];
                for fraction in [0.001,0.1,0.5,1.0] {
                    let distance=dx.hypot(dy)*fraction;
                    let count=(distance/texels/4.0).ceil().clamp(1.0,16.0);
                    let needed=(distance/count).max(texels).max(1.0).log2().floor() as u32+1;
                    assert!(bound>=needed);
                }
            }}
        }
    }
}
