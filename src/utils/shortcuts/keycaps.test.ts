import {expect,it} from 'vitest';
import {shortcutLayout,shortcutLabels} from './keycaps';
import {DEFAULT_SHORTCUT_SETTINGS} from './settings';
import {shortcutIntervals,type ShortcutInterval} from './intervals';
it('keeps complete chords inside portrait and landscape output at extreme placement and scale',()=>{
  for(const [width,height] of [[1920,1080],[1080,1920],[320,180]])for(const x of [0,0.5,1])for(const y of [0,0.5,1]) {
    const caps=shortcutLayout(['⌃','⌥','⇧','⌘','fn','BACKSPACE'],{...DEFAULT_SHORTCUT_SETTINGS,x,y,size:0.15},width,height);
    for(const cap of caps){expect(cap.x).toBeGreaterThanOrEqual(0);expect(cap.y).toBeGreaterThanOrEqual(0);
      expect(cap.x+cap.width).toBeLessThanOrEqual(width+1e-8);expect(cap.y+cap.height).toBeLessThanOrEqual(height+1e-8);}
    expect(caps[1].x).toBeGreaterThan(caps[0].x+caps[0].width);
  }
});
it('formats captured labels without changing source identity',()=>{
  const interval=shortcutIntervals([{timestamp:100,type:'down',key:'é',modifiers:['command','shift']}])[0];
  expect(shortcutLabels(interval)).toEqual(['⇧','⌘','É']);
  expect(interval.key).toBe('é');
});

it('shows one Caps Lock keycap when its own modifier flag is also active',()=>{
  const interval:ShortcutInterval={key:'CapsLock',modifiers:['capsLock'],firstDown:0,lastDown:0,lastEvent:0,start:0,end:1500,presses:1};
  expect(shortcutLabels(interval)).toEqual(['⇪']);
  expect(shortcutLabels({...interval,modifiers:['command','capsLock']})).toEqual(['⌘','⇪']);
});
it('normalizes shifted number-row symbols without hiding command-chord modifiers',()=>{
 const base={firstDown:0,lastDown:0,lastEvent:0,start:0,end:1500,presses:1};
 for(let i=0;i<10;i++) {
  const key='!@#$%^&*()'[i];
  expect(shortcutLabels({...base,key,modifiers:['shift']})).toEqual([key]);
  expect(shortcutLabels({...base,key,modifiers:['shift','command']})).toEqual(['⇧','⌘','1234567890'[i]]);
 }
});
