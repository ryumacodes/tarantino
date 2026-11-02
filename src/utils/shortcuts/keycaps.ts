import type {ShortcutInterval} from './intervals';
import type {ShortcutSettings} from './settings';
const symbols:Record<string,string>={command:'⌘',control:'⌃',option:'⌥',shift:'⇧',capsLock:'⇪',function:'fn',
  Enter:'↵',Tab:'⇥',Backspace:'⌫',Delete:'⌦',ArrowLeft:'←',ArrowRight:'→',ArrowUp:'↑',ArrowDown:'↓',
  Escape:'esc',CapsLock:'⇪',' ':'space',PageUp:'⇞',PageDown:'⇟',Home:'↖',End:'↘'};
export function shortcutLabels(interval:ShortcutInterval):string[] {
  let key=interval.key;
  let modifiers=interval.modifiers.filter(m=>m!=='capsLock'||key!=='CapsLock');
  const shiftedDigit='!@#$%^&*()'.indexOf(key);
  if(key.length===1&&shiftedDigit>=0&&modifiers.includes('shift')) {
    // A shifted symbol alone is legible as the symbol; a command chord names
    // the underlying digit while retaining its explicit Shift modifier.
    if(modifiers.length===1)modifiers=[];
    else key='1234567890'[shiftedDigit];
  }
  return [...modifiers.map(m=>symbols[m]),symbols[key]??key.toUpperCase()];
}
export function shortcutLayout(labels:readonly string[],settings:ShortcutSettings,width:number,height:number) {
  const initial=height*settings.size,gap=initial*0.12,padding=width*0.025;
  const widths=labels.map(label=>initial*Math.max(1,0.55+label.length*0.48));
  const total=widths.reduce((sum,w)=>sum+w,0)+gap*Math.max(0,labels.length-1);
  const scale=total>0?Math.min(1,Math.max(0,width-padding*2)/total):1;
  const h=initial*scale,w=total*scale;
  let x=Math.min(width-padding-w,Math.max(padding,width*settings.x-w/2));
  const y=Math.min(height-h-height*0.025,Math.max(height*0.025,height*settings.y-h/2));
  return labels.map((label,index)=>{const cap={label,x,y,width:widths[index]*scale,height:h};x+=cap.width+gap*scale;return cap;});
}
/** Shared raster drawing so export can reuse the same keycap appearance. */
export function drawShortcutKeycaps(ctx:CanvasRenderingContext2D,interval:ShortcutInterval,
  settings:ShortcutSettings,width:number,height:number) {
  const caps=shortcutLayout(shortcutLabels(interval),settings,width,height);
  drawKeycaps(ctx,caps);
}

export function drawKeycaps(ctx:CanvasRenderingContext2D,caps:readonly {label:string;x:number;y:number;width:number;height:number;opacity?:number}[]) {
  ctx.save();
  for(const cap of caps) {
    const alpha=Math.max(0,Math.min(1,cap.opacity??1));
    if(alpha===0)continue;
    ctx.globalAlpha=alpha;
    const {x,y,width:w,height:h}=cap;
    const radius=h*0.2;
    ctx.beginPath();ctx.roundRect(x,y,w,h,radius);
    ctx.fillStyle='#201e1b';ctx.shadowColor='#0008';ctx.shadowBlur=h*0.2;ctx.shadowOffsetY=h*0.07;ctx.fill();
    ctx.shadowColor='transparent';ctx.lineWidth=Math.max(1,h*0.025);ctx.strokeStyle='#746858';ctx.stroke();
    ctx.fillStyle='#fae7c5';ctx.font=`500 ${h*0.48}px -apple-system, BlinkMacSystemFont, sans-serif`;
    ctx.textAlign='center';ctx.textBaseline='middle';ctx.fillText(cap.label,x+w/2,y+h*0.53,w-h*0.24);
  }
  ctx.restore();
}
