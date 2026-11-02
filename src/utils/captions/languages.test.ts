import {expect,it} from 'vitest';
import {speechLocale,speechLanguageName,speechLanguages} from './languages';
it('normalizes Apple locale identifiers without duplicate selections',()=>{
 expect(speechLocale('en_US')).toBe('en-US');
 expect(speechLanguages('en-US',['en_US','fr_FR','en-US'])).toEqual(['en-US','fr-FR']);
 expect(speechLanguageName('en_US')).toBe('English (United States)');
 expect(speechLanguageName('bad_locale!')).toBe('bad_locale!');
});
