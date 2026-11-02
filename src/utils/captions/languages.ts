/** Apple locale identifiers use underscores; Intl and HTML options use BCP 47. */
export function speechLocale(identifier:string):string {
  try{return Intl.getCanonicalLocales(identifier.replace(/_/g,'-'))[0]??identifier;}
  catch{return identifier;}
}
const names=new Intl.DisplayNames(['en'],{type:'language',languageDisplay:'standard'});
export function speechLanguageName(identifier:string):string {
  const locale=speechLocale(identifier);
  try{return names.of(locale)??identifier;}catch{return identifier;}
}
export function speechLanguages(selected:string,supported:readonly string[]):string[] {
  return [...new Set([selected,...supported].map(speechLocale))]
    .sort((a,b)=>speechLanguageName(a).localeCompare(speechLanguageName(b)));
}
