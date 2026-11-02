/** Rounded continuous corners, sampled radially for the compositor's distance field. */
export interface CaptionCorner {radius:number;curvature:number[]}
export function captionCorner(width:number,height:number,radius=12):CaptionCorner {
  if(![width,height,radius].every(value=>Number.isFinite(value)&&value>=0))throw new RangeError('Invalid caption corner');
  const side=Math.min(width,height),r=Math.min(radius,side/2);
  if(r===0)return {radius:0,curvature:Array(16).fill(1)};
  const extent=Math.min(r*2,side/2);
  // When the corner approaches a capsule, reduce the tangent transition until circular.
  const turn=Math.PI/4*Math.min(1,Math.max(0,2-4*r/side));
  const tangent=r*Math.tan(turn/2);
  const dx=tangent*Math.cos(turn),dy=tangent*Math.sin(turn);
  const chord=r*Math.SQRT2*Math.sin(Math.PI/4-turn);
  const third=(extent-chord-dx-dy)/3;
  const endX=3*third+dx;
  const point=(t:number)=>({
    x:3*(1-t)*(1-t)*t*(2*third)+3*(1-t)*t*t*(3*third)+t*t*t*endX,
    y:t*t*t*dy,
  });
  const curvature=Array.from({length:16},(_,index)=>{
    if(index===0)return 1;
    const angle=index/15*Math.PI/4,sin=Math.sin(angle),cos=Math.cos(angle);
    const cross=(x:number,y:number)=>x*cos+(y-extent)*sin;
    let distance:number;
    if(cross(endX,dy)>=-1e-12&&third>1e-12) {
      let low=0,high=1;
      for(let iteration=0;iteration<48;iteration++){
        const mid=(low+high)/2,p=point(mid);
        if(cross(p.x,p.y)<0)low=mid;else high=mid;
      }
      const p=point((low+high)/2);
      distance=p.x*sin+(extent-p.y)*cos;
    } else {
      // Ray-circle intersection for the central arc, measured from the corner origin.
      const cx=extent-r,cy=r-extent;
      const projection=cx*sin-cy*cos;
      distance=projection+Math.sqrt(Math.max(0,projection*projection-cx*cx-cy*cy+r*r));
    }
    return Math.fround(distance/extent);
  });
  return {radius:extent,curvature};
}
