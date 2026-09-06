export const isLinuxRuntime = typeof navigator !== 'undefined'
  && /Linux/i.test(`${navigator.platform} ${navigator.userAgent}`);
