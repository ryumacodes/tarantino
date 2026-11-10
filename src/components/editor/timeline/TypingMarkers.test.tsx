import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it, vi } from 'vitest';
const mocks = vi.hoisted(() => ({state: {} as any}));
vi.mock('../../../stores/editor', () => ({useEditorStore: (selector: any) => selector(mocks.state)}));
import TypingMarkers from './TypingMarkers';

it('positions typing markers on the edited clock and excludes trimmed or offscreen ranges', () => {
  mocks.state={typingRanges:[{start:2000,end:5000}],dismissedTypingRanges:[],videoFilePath:'/take.mp4',
    tracks:[{visible:true,locked:false,clips:[{id:'take',type:'video',enabled:true,locked:false,
      sourceIn:1000,sourceOut:7000,startTime:4000,playbackRate:2}]}]};
  const props={pixelsPerMs:.1,viewportLeft:0,viewportRight:1000,trimStart:0,trimEnd:9000,isExporting:false};
  const markup=renderToStaticMarkup(<TypingMarkers {...props}/>);
  expect(markup).toContain('Edit typing section at 4.5 seconds');
  expect(markup).toContain('left:450px;width:150px');
  expect(renderToStaticMarkup(<TypingMarkers {...props} trimStart={5000}/>)).toContain('left:500px;width:100px');
  expect(renderToStaticMarkup(<TypingMarkers {...props} viewportLeft={700}/>)).not.toContain('aria-haspopup');
  expect(renderToStaticMarkup(<TypingMarkers {...props} trimEnd={4000}/>)).not.toContain('aria-haspopup');
  expect(renderToStaticMarkup(<TypingMarkers {...props} isExporting/>)).toContain('disabled=""');
  mocks.state.dismissedTypingRanges=[2000];
  expect(renderToStaticMarkup(<TypingMarkers {...props}/>)).toBe('');
});
