export const isLinuxRuntime = /Linux/i.test(`${navigator.platform} ${navigator.userAgent}`);
export const isMacRuntime = /Mac/i.test(navigator.platform);
