/** Upper mip needed by the rim shader, whose optical LOD is capped at four. */
export function loupeMipLevel(zoom:number) {
  const sigma=0.58*(2**4-1)/zoom*0.5;
  const spread=0.6*Math.max(0,Math.min(1,1-(Math.log2(sigma)-1)/3));
  const ring=0.5+0.4*spread,jitter=1.6*spread;
  const variance=1/6+ring*ring*(1+jitter*jitter/12)*0.5;
  let spanSquared=(sigma*sigma+0.75)/(0.25+variance);
  if(spanSquared<4)spanSquared=(sigma*sigma+1/12)/(1/12+variance);
  // Both trilinear neighbors are read, including at integer LOD boundaries.
  return Math.floor(Math.max(0,Math.log2(Math.sqrt(spanSquared))))+1;
}
