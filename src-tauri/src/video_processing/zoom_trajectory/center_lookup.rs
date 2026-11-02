//! Resolve sorted caret/anchor centers without rescanning a typing session per frame.
use crate::video_processing::types::ZoomCenter;

pub(crate) fn resolve_center_at_time(
    centers:&[ZoomCenter],time_ms:f64,fallback_x:f64,fallback_y:f64,interpolate:bool,
)->(f64,f64) {
    let Some(first)=centers.first() else {return (fallback_x,fallback_y)};
    if time_ms<=first.time as f64 {return (first.x,first.y);}
    // Keep the established invalid-time behavior of the linear resolver.
    if time_ms.is_nan() {let last=centers.last().unwrap();return (last.x,last.y);}
    let next=centers.partition_point(|center|(center.time as f64)<time_ms);
    let Some(next_center)=centers.get(next) else {
        let last=centers.last().unwrap();return (last.x,last.y);
    };
    let previous=&centers[next-1];
    if !interpolate || next_center.time<=previous.time {return (previous.x,previous.y);}
    let progress=((time_ms-previous.time as f64)/(next_center.time-previous.time) as f64).clamp(0.0,1.0);
    (previous.x+(next_center.x-previous.x)*progress,previous.y+(next_center.y-previous.y)*progress)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn linear(centers:&[ZoomCenter],time:f64,interpolate:bool)->(f64,f64) {
        if centers.is_empty() {return (0.4,0.6);}
        let first=&centers[0];
        if time<=first.time as f64 {return (first.x,first.y);}
        for pair in centers.windows(2) {
            let a=&pair[0];let b=&pair[1];
            if time<=b.time as f64 {
                if !interpolate || b.time<=a.time {return (a.x,a.y);}
                let ratio=((time-a.time as f64)/(b.time-a.time) as f64).clamp(0.0,1.0);
                return (a.x+(b.x-a.x)*ratio,a.y+(b.y-a.y)*ratio);
            }
        }
        let last=centers.last().unwrap();(last.x,last.y)
    }
    fn centers(count:usize)->Vec<ZoomCenter> {
        (0..count).map(|i|ZoomCenter{time:(i/3*137) as u64,x:(i%31) as f64/30.0,y:(i%17) as f64/16.0}).collect()
    }
    #[test]
    fn bounded_lookup_matches_linear_caret_history_including_duplicates_and_seeks() {
        for count in [0,1,2,3,31,1000] {
            let centers=centers(count);
            let mut times=vec![-1.0,0.0,f64::INFINITY,f64::NEG_INFINITY,f64::NAN,100_000.0];
            for center in &centers {times.extend([center.time as f64-0.001,center.time as f64,center.time as f64+0.001]);}
            times.reverse();
            for interpolate in [false,true] {for &time in &times {
                assert_eq!(resolve_center_at_time(&centers,time,0.4,0.6,interpolate),linear(&centers,time,interpolate),
                    "count={count},time={time},interpolate={interpolate}");
            }}
        }
    }
    #[test]
    #[ignore="local caret lookup performance workload"]
    fn long_typing_history_lookup_profile() {
        let centers=centers(10_000);
        let times:Vec<_>=(0..30_000).map(|i|(i*15401%450_000) as f64+0.25).collect();
        let start=std::time::Instant::now();
        let baseline:Vec<_>=times.iter().map(|&t|linear(&centers,t,true)).collect();
        let linear_time=start.elapsed();
        let start=std::time::Instant::now();
        let bounded:Vec<_>=times.iter().map(|&t|resolve_center_at_time(&centers,t,0.4,0.6,true)).collect();
        let bounded_time=start.elapsed();
        assert_eq!(baseline,bounded);
        println!("10000 centers,30000 seeks: linear={linear_time:?}, bounded={bounded_time:?}");
    }
}
