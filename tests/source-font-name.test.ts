import {expect,it} from "vitest";
import {sourceFontNameKey} from "../src/editor/source-font-name";
it.each([
 ["ABCDEF+LocalSerif-Bold","Local Serif Bold"],
 ["Local_Serif,Bold","local serif bold"],
 ["École Serif","E\u0301cole-Serif"],
 [" Source Serif ","source-serif"],
])("matches only normalized full names: %s",(source,installed)=>{expect(sourceFontNameKey(source)).toBe(sourceFontNameKey(installed));});
it.each([
 ["LocalSerif-Bold","LocalSerif"],
 ["LocalSerif-BoldItalic","LocalSerif-Bold"],
 ["LocalSerif","LocalSerifDisplay"],
 ["abcDEF+LocalSerif","LocalSerif"],
 ["ABCDE+LocalSerif","LocalSerif"],
 ["ABCDEFG+LocalSerif","LocalSerif"],
 ["ABCDEF+GHIJKL+LocalSerif","LocalSerif"],
 ["Source.Serif","SourceSerif"],
])("does not infer aliases or strip malformed tags: %s",(source,installed)=>{expect(sourceFontNameKey(source)).not.toBe(sourceFontNameKey(installed));});
it.each([undefined,"","  ","_- ,"])("has no match key for empty name %j",name=>{expect(sourceFontNameKey(name)).toBe("");});
