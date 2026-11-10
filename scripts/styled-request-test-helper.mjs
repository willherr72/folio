// Opt-in native integration bridge: exercise the actual editor request builder.
import {createServer} from 'vite';
let text='';for await(const chunk of process.stdin)text+=chunk;
const server=await createServer({logLevel:'silent',server:{middlewareMode:true}});
try{const {createStyledEditor,styledEditorRequest}=await server.ssrLoadModule('/src/editor/styled-paragraph-editor.ts');process.stdout.write(JSON.stringify(styledEditorRequest(createStyledEditor(JSON.parse(text)))));}finally{await server.close();}
