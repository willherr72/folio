/** Name hints only: a matching name does not establish equivalent outlines. */
export function sourceFontNameKey(name?:string):string {
 return (name??"").normalize("NFC").trim().replace(/^[A-Z]{6}\+/,"").toLowerCase().replace(/[\s,_-]+/g,"");
}
