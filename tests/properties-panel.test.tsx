import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import {PropertiesPanel} from "../src/components/PropertiesPanel";
function panel(contextKey:string,autoExpand=false){return <PropertiesPanel contextKey={contextKey} autoExpand={autoExpand}><textarea aria-label="Draft" defaultValue="Keep this"/></PropertiesPanel>;}
afterEach(cleanup);
it("starts collapsed for reading, opens a new editing context and remembers deliberate collapse",()=>{
 const view=render(panel("one:page"));const toggle=screen.getByRole("button",{name:"Properties"});expect(toggle).toHaveAttribute("aria-expanded","false");
 view.rerender(panel("one:text",true));expect(toggle).toHaveAttribute("aria-expanded","true");fireEvent.click(toggle);expect(toggle).toHaveAttribute("aria-expanded","false");
 view.rerender(panel("one:text",true));expect(toggle).toHaveAttribute("aria-expanded","false");view.rerender(panel("two:text",true));expect(toggle).toHaveAttribute("aria-expanded","true");
});
it("keeps the editor DOM and draft through collapse/expand and provides a matching controls target",()=>{
 render(panel("one:text",true));const input=screen.getByRole("textbox"),toggle=screen.getByRole("button",{name:"Properties"});fireEvent.change(input,{target:{value:"Uncommitted draft"}});
 fireEvent.click(toggle);fireEvent.click(toggle);expect(screen.getByRole("textbox")).toBe(input);expect(input).toHaveValue("Uncommitted draft");expect(document.getElementById(toggle.getAttribute("aria-controls")!)).toContainElement(input);
});
it("reopens when wide-layout editing focuses a previously collapsed context",()=>{
 render(panel("one:text",true));const toggle=screen.getByRole("button",{name:"Properties"});fireEvent.click(toggle);expect(toggle).toHaveAttribute("aria-expanded","false");fireEvent.focus(screen.getByRole("textbox"));expect(toggle).toHaveAttribute("aria-expanded","true");
});
it("discards a previous collapse after selecting another context, even without toggling it",()=>{
 const view=render(panel("one:draw",true));const toggle=screen.getByRole("button",{name:"Properties"});fireEvent.click(toggle);expect(toggle).toHaveAttribute("aria-expanded","false");
 view.rerender(panel("one:page"));expect(toggle).toHaveAttribute("aria-expanded","false");view.rerender(panel("one:draw",true));expect(toggle).toHaveAttribute("aria-expanded","true");
});
