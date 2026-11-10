import { invoke } from "@tauri-apps/api/core";
import type { PagePlan } from "./types";
export interface PrintOptions { scale: "fit" | "actual"; orientation: "portrait" | "landscape" }
export const printPdf = (pages:PagePlan[],options:PrintOptions) => invoke<boolean>("print_pdf",{request:{pages},options});
export {selectPages as selectPrintPages} from "./page-selection";
