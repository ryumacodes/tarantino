import { zoomPresentationUpdate } from '../motion/zoomPresentation';
import { expect, it, vi } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { useEditorStore } from '../../stores/editor';
import { createProjectDocument, parseProjectDocument } from './document';

function fixture() {
  const state = useEditorStore.getState();
  return { ...state, videoFilePath: '/recording.mp4', projectTitle: 'Edited take',
    visualSettings: { ...state.visualSettings, screenMotionStyle: undefined, cursorMotionStyle: 'none' as const, zoomLeadInMs: 650 },
    clips: [{ id: 'clip', name: 'Cut', type: 'video' as const, trackId: 'track', startTime: 0, duration: 500,
      sourceIn: 1000, sourceOut: 2000, sourceFilePath: '/recording.mp4', enabled: true, locked: false,
      color: '#ffffff', playbackRate: 2, hideCursor: true }],
    tracks: [{ id: 'track', name: 'Video', type: 'video' as const, clips: [], height: 80,
      visible: true, muted: false, solo: false, locked: false, order: 0 }],
    overlays: [{ id: 'label', type: 'text' as const, startTime: 200, endTime: 900, text: 'Hello', x: 0.2, y: 0.3 }] as any,
  };
}
it('keeps saved cursor rotation choices when new-project motion defaults change', () => {
  const state = fixture();
  expect(useEditorStore.getInitialState().visualSettings).toMatchObject({
    rotateCursorWhileMoving: true, rotationIntensity: 50, cursorRotationMode: 'horizontal',
  });
  for (const rotation of [
    { rotateCursorWhileMoving: false, rotationIntensity: 30, cursorRotationMode: 'horizontal' as const },
    { rotateCursorWhileMoving: true, rotationIntensity: 75, cursorRotationMode: 'directional' as const },
  ]) {
    const document = createProjectDocument({ ...state, visualSettings: { ...state.visualSettings, ...rotation } });
    const restored = parseProjectDocument(JSON.parse(JSON.stringify(document)));
    expect(restored.state.visualSettings).toMatchObject(rotation);
  }
});

it('keeps saved click effects while new projects start without a highlight', () => {
  const state = fixture();
  expect(useEditorStore.getInitialState().visualSettings.clickEffect).toBe('none');
  for (const clickEffect of ['none', 'circle', 'ripple'] as const) {
    const document = createProjectDocument({ ...state, visualSettings: { ...state.visualSettings, clickEffect } });
    expect(parseProjectDocument(JSON.parse(JSON.stringify(document))).state.visualSettings.clickEffect).toBe(clickEffect);
  }
});

it('preserves independent blur strengths including explicit zero values', () => {
  const state = fixture();
  const strengths = { motionBlurPanIntensity: 20, motionBlurZoomIntensity: 0, motionBlurCursorIntensity: 0 };
  const document = createProjectDocument({ ...state, visualSettings: { ...state.visualSettings, ...strengths } });
  expect(parseProjectDocument(JSON.parse(JSON.stringify(document))).state.visualSettings).toMatchObject(strengths);
});

it('preserves saved jump filtering choices when the default threshold changes', () => {
  const state = fixture();
  for (const filtering of [
    { removeCursorShakes: true, shakesThreshold: 3 },
    { removeCursorShakes: false, shakesThreshold: 500 },
    { removeCursorShakes: true, shakesThreshold: 900 },
  ]) {
    const document = createProjectDocument({ ...state, visualSettings: { ...state.visualSettings, ...filtering } });
    expect(parseProjectDocument(JSON.parse(JSON.stringify(document))).state.visualSettings).toMatchObject(filtering);
  }
});

it('preserves saved end freezes independently of their enabled state', () => {
  const state = fixture();
  for (const freeze of [
    { stopCursorAtEnd: false, stopCursorDuration: 500 },
    { stopCursorAtEnd: true, stopCursorDuration: 1200 },
  ]) {
    const document = createProjectDocument({ ...state, visualSettings: { ...state.visualSettings, ...freeze } });
    expect(parseProjectDocument(JSON.parse(JSON.stringify(document))).state.visualSettings).toMatchObject(freeze);
  }
});

it('round trips edits without serializing actions, playback state or caches', () => {
  const document = createProjectDocument(fixture());
  const reopened = parseProjectDocument(JSON.parse(JSON.stringify(document)));
  expect(reopened.state.clips[0]).toMatchObject({ sourceIn: 1000, sourceOut: 2000, playbackRate: 2, hideCursor: true });
  expect(reopened.state.tracks[0].clips[0]).toBe(reopened.state.clips[0]);
  expect(reopened.state.overlays).toEqual(document.state.overlays);
  expect(reopened.state.visualSettings.screenMotionStyle).toBeUndefined();
  expect(reopened.state.visualSettings.cursorMotionStyle).toBe('none');
  expect(reopened.state.visualSettings.zoomLeadInMs).toBe(650);
  for (const key of ['history', 'isPlaying', 'initializeEditor', 'mouseEvents', 'thumbnails']) expect(key in reopened.state).toBe(false);
});
it('restores only document fields even when extra keys are present', () => {
  const document: any = createProjectDocument(fixture());
  document.state.initializeEditor = 'injected';
  expect('initializeEditor' in parseProjectDocument(document).state).toBe(false);
});
it('rejects unknown versions, malformed motion styles and invalid clip references', () => {
  const document: any = createProjectDocument(fixture());
  expect(() => parseProjectDocument({ ...document, version: 99 })).toThrow('version');
  document.state.visualSettings.screenMotionStyle = 'broken';
  expect(() => parseProjectDocument(document)).toThrow('screen style');
  document.state.visualSettings.screenMotionStyle = 'smooth';
  document.state.clips[0].trackId = 'missing';
  expect(() => parseProjectDocument(document)).toThrow('missing track');
});
it('rejects incomplete documents rather than replacing editor fields with undefined', () => {
  const document: any = createProjectDocument(fixture());
  delete document.state.audioSettings;
  expect(() => parseProjectDocument(document)).toThrow('audioSettings');
});

it('reopens camera layouts and a trim extended by slow motion', () => {
  const state=fixture();
  state.duration=2000; state.trimStart=0; state.trimEnd=4000;
  state.clips[0]={...state.clips[0],duration:4000,sourceIn:0,sourceOut:2000,playbackRate:0.5};
  state.visualSettings={...state.visualSettings,webcamLayout:'fullscreen',webcamMirrored:true};
  state.webcamKeyframes=[{id:'hide',time:500,x:0.8,y:0.8,size:0.15,shape:'circle',visible:false,layout:'hidden'}];
  const document=createProjectDocument(state);
  const restored=parseProjectDocument(document);
  expect(restored.state.webcamKeyframes).toEqual(state.webcamKeyframes);
  expect(restored.state.visualSettings.webcamLayout).toBe('fullscreen');
  expect(restored.state.visualSettings.webcamMirrored).toBe(true);
  expect(restored.state.trimEnd).toBe(4000);
  (document.state.webcamKeyframes[0] as any).layout='invalid';
  expect(()=>parseProjectDocument(document)).toThrow('camera layout');
});

it('rejects invalid project zoom anticipation values', () => {
  for (const value of [-1, 2001, NaN, Infinity, '500']) {
    const doc: any = createProjectDocument(fixture());
    doc.state.visualSettings.zoomLeadInMs = value;
    expect(() => parseProjectDocument(doc)).toThrow('zoom lead-in');
  }
});

it('preserves independent audio mutes and accepts older projects without them', () => {
  const doc = createProjectDocument(fixture());
  doc.state.audioSettings.micMuted = true;
  doc.state.audioSettings.systemMuted = false;
  const restored = parseProjectDocument(JSON.parse(JSON.stringify(doc)));
  expect(restored.state.audioSettings).toMatchObject({ micMuted: true, systemMuted: false });
  delete doc.state.audioSettings.micMuted;
  delete doc.state.audioSettings.systemMuted;
  expect(() => parseProjectDocument(doc)).not.toThrow();
  (doc.state.audioSettings as any).micMuted = 'false';
  expect(() => parseProjectDocument(doc)).toThrow('audio micMuted');
});

it('round trips source crop and rejects rectangles outside the recording', () => {
  const state=fixture();
  state.videoWidth=128; state.videoHeight=64;
  const crop={x:0.5,y:0,width:0.5,height:1};
  const document=createProjectDocument({...state,visualSettings:{...state.visualSettings,sourceCrop:crop}});
  expect(parseProjectDocument(document).state.visualSettings.sourceCrop).toEqual(crop);
  document.state.visualSettings.sourceCrop={...crop,width:0.75};
  expect(()=>parseProjectDocument(document)).toThrow('inside the source');
});

it('preserves disabled zoom edits and rejects malformed enable state', () => {
  const state = fixture();
  state.zoomAnalysis = { zoom_blocks: [{ id: 'disabled', start_time: 100, end_time: 900,
    center_x: 0.4, center_y: 0.6, click_x: 0.4, click_y: 0.6, zoom_factor: 2,
    is_manual: true, is_disabled: true, loupe: { radius: 0.25, glass: true, bevel: 0.1,
      chromatic_aberration: 0.01, shadow_alpha: 0.35 } }], total_clicks: 0, session_duration: 1000,
    config: { enabled: true, zoom_factor: 2, zoom_duration: 800, min_click_spacing: 100 } };
  const document = createProjectDocument(state);
  const reopened = parseProjectDocument(JSON.parse(JSON.stringify(document)));
  expect(reopened.state.zoomAnalysis?.zoom_blocks).toEqual(state.zoomAnalysis!.zoom_blocks);
  const block = document.state.zoomAnalysis!.zoom_blocks[0];
  block.loupe!.radius = 0;
  expect(() => parseProjectDocument(document)).toThrow('magnifier bounds');
  block.loupe!.radius = 0.25;
  (block as any).is_disabled = 'yes';
  expect(() => parseProjectDocument(document)).toThrow('zoom enabled state');
});

it('persists independent clip audio levels and rejects unsafe gains', () => {
  const state = fixture();
  Object.assign(state.clips[0], { micVolume: 0.25, systemVolume: 0.7 });
  const saved = createProjectDocument(state);
  expect(parseProjectDocument(saved).state.clips[0]).toMatchObject({ micVolume: 0.25, systemVolume: 0.7 });
  for (const key of ['micVolume', 'systemVolume']) {
    for (const invalid of [-1, 1.01, NaN, Infinity, '1']) {
      const broken = structuredClone(saved) as any;
      broken.state.clips[0][key] = invalid;
      expect(() => parseProjectDocument(broken)).toThrow(key);
    }
  }
});

it('round trips clip smoothing overrides and rejects nonboolean flags', () => {
  const state=fixture();
  const document:any=createProjectDocument({...state,clips:state.clips.map(c=>({...c,disableCursorSmoothing:true}))});
  expect(parseProjectDocument(JSON.parse(JSON.stringify(document))).state.clips[0].disableCursorSmoothing).toBe(true);
  document.state.clips[0].disableCursorSmoothing='true';
  expect(()=>parseProjectDocument(document)).toThrow('cursor smoothing');
});
it('round trips background audio and rejects invalid soundtrack settings',()=>{
  const state=fixture();
  state.audioSettings={...state.audioSettings,backgroundAudio:{path:'/assets/music.wav',name:'Music',volume:0.3,muted:true}};
  const document=createProjectDocument(state);
  expect(parseProjectDocument(document).state.audioSettings.backgroundAudio).toEqual(state.audioSettings.backgroundAudio);
  const invalid=JSON.parse(JSON.stringify(document));invalid.state.audioSettings.backgroundAudio.volume=-1;
  expect(()=>parseProjectDocument(invalid)).toThrow('background audio');
});

it('saves native automatic zoom blocks with a null magnifier', () => {
  const state = fixture();
  state.zoomAnalysis = { zoom_blocks: [{ id: 'typing_zoom_0', start_time: 0, end_time: 500,
    center_x: 0.5, center_y: 0.5, click_x: 0.5, click_y: 0.5, zoom_factor: 2,
    is_manual: false, loupe: null }], total_clicks: 0, session_duration: 500,
    config: { enabled: true, zoom_factor: 2, zoom_duration: 800, min_click_spacing: 100 } } as any;
  const document = createProjectDocument(state);
  expect(parseProjectDocument(JSON.parse(JSON.stringify(document))).state.zoomAnalysis?.zoom_blocks[0].loupe).toBeNull();
  for (const invalid of [false, 0, '', {}, { glass: true }]) {
    (document.state.zoomAnalysis!.zoom_blocks[0] as any).loupe = invalid;
    expect(() => parseProjectDocument(document)).toThrow('magnifier');
  }
});


it('round-trips dormant magnifier settings and validates them', () => {
  const state=fixture();
  const lens={radius:0.6,glass:true,bevel:0.4,chromatic_aberration:0.7,shadow_alpha:0.2};
  state.zoomAnalysis={total_clicks:0,session_duration:1000,
    config:{enabled:true,zoom_factor:2,zoom_duration:1000,min_click_spacing:100},
    zoom_blocks:[{id:'zoom',start_time:0,end_time:1000,zoom_factor:2,
      click_x:0.5,click_y:0.5,center_x:0.5,center_y:0.5,is_manual:true,loupe:null,inactive_loupe:lens}]};
  const document=createProjectDocument(state);
  const reopened=parseProjectDocument(JSON.parse(JSON.stringify(document)));
  const block=reopened.state.zoomAnalysis!.zoom_blocks[0];
  expect(block.loupe).toBeNull();
  expect(zoomPresentationUpdate(block,'magnifier').loupe).toEqual(lens);
  document.state.zoomAnalysis!.zoom_blocks[0].inactive_loupe!.radius=0;
  expect(()=>parseProjectDocument(document)).toThrow('magnifier bounds');
});
