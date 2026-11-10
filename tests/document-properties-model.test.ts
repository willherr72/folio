import {it,expect} from 'vitest';
import {validateMetadata,propertiesDraft,metadataFromDraft,metadataEqual,copyDocumentProperties} from '../src/editor/document-properties';
const source={title:'Original',author:'Author',subject:'Subject',keywords:'Keywords'};
it('preserves untouched legacy control characters while validating changed fields',()=>{
 const legacy={...source,author:'Old\u0001author'};
 expect(metadataFromDraft(legacy,undefined,{...legacy,title:'Public title'})).toEqual({mode:'update',title:'Public title'});
 expect(()=>metadataFromDraft(legacy,undefined,{...legacy,author:'New\u0001author'})).toThrow();
});
it('merges new fields with existing explicit edits and keeps source untouched',()=>{
 const pending={mode:'update' as const,title:'Edited'};const draft=propertiesDraft(source,pending);expect(draft.title).toBe('Edited');draft.author='New author';
 expect(metadataFromDraft(source,pending,draft)).toEqual({mode:'update',title:'Edited',author:'New author'});expect(source.title).toBe('Original');expect(pending).toEqual({mode:'update',title:'Edited'});
});
it('editing after removal does not resurrect custom or unedited original metadata',()=>{
 const pending={mode:'remove' as const};const draft=propertiesDraft(source,pending);expect(Object.values(draft)).toEqual(['','','','']);draft.title='Public title';
 expect(metadataFromDraft(source,pending,draft)).toEqual({mode:'update',clearExisting:true,title:'Public title'});
 expect(propertiesDraft(source,{mode:'update',clearExisting:true,title:'Public title'})).toEqual({title:'Public title',author:'',subject:'',keywords:''});
});
it('preserves deliberate empty edits and recognizes no-op or equivalent drafts',()=>{
 expect(metadataFromDraft(source,undefined,{...source})).toBeUndefined();expect(metadataFromDraft(source,undefined,{...source,title:''})).toEqual({mode:'update',title:''});
 expect(metadataFromDraft(source,undefined,source,true)).toEqual({mode:'remove'});
 expect(metadataEqual({mode:'update',title:'A',author:'B'},{mode:'update',author:'B',title:'A',clearExisting:false})).toBe(true);
 expect(metadataEqual({mode:'remove'},undefined)).toBe(false);
});
it('bounds Unicode and rejects unknown actions or ignored fields',()=>{
 for(const value of [{mode:'remove',title:'ignored'},{mode:'update'},{mode:'other'},null,[],{mode:'update',title:3},{mode:'update',title:'\ud800'},{mode:'update',title:'\u0000'},{mode:'update',title:'é'.repeat(8193)}])expect(()=>validateMetadata(value)).toThrow();
 for(const value of [{mode:'remove'},{mode:'update',title:''},{mode:'update',title:'é'.repeat(8192)},{mode:'update',title:'文書😀\n\r\t',clearExisting:true}])expect(()=>validateMetadata(value)).not.toThrow();
});
it('copies inspected provider data and refuses malformed or oversized results',()=>{
 const value={...source,creator:'Creator',producer:'Producer',creationDate:'Date',modificationDate:'Date',hasInfo:true,hasXmp:true,conflicts:['title'],multipleAuthors:false,error:null};
 const copy=copyDocumentProperties(value);value.conflicts.push('author');value.title='mutated';expect(copy.title).toBe('Original');expect(copy.conflicts).toEqual(['title']);
 expect(()=>copyDocumentProperties({...value,title:'x'.repeat(16385)})).toThrow();expect(()=>copyDocumentProperties({...value,conflicts:['foreign']})).toThrow();
});
