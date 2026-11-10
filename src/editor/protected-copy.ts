import type {PagePlan,ProtectionInfo} from './types';
export type PdfPermissions=ProtectionInfo['permissions'];
export type ProtectionChange={mode:'remove'}|{mode:'passwords';userPassword:string;ownerPassword:string;permissions:PdfPermissions};
export const defaultPdfPermissions:PdfPermissions={print:true,printHighQuality:true,copyContent:true,editContent:true,annotate:true,fillForms:true,assemble:true};
export function protectionChangeDenied(pages:PagePlan[]):string|null{
 if(!pages.length||pages.length>10000)return 'Choose between 1 and 10,000 pages.';
 return pages.some(page=>page.protection&&!page.protection.owner)?'Use Unlock editing with the owner password for every protected PDF before changing security.':null;
}
