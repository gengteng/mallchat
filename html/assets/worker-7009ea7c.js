(function(){"use strict";const s=({type:t,value:o})=>{self.postMessage(JSON.stringify({type:t,value:o}))};let e,r=null;const f=100;let n=0,a=null,l=!1;const c=t=>{e==null||e.send(JSON.stringify(t))},g=()=>{r=setInterval(()=>{c({type:2})},9900)},y=()=>{r&&(clearInterval(r),r=null)},i=()=>{if(y(),!l){if(l=!0,a&&(clearTimeout(a),a=null),n>=f){n=0;return}a=setTimeout(()=>{p(),n++,l=!1},2e3)}},u=()=>{i(),s({type:"error"})},v=()=>{i(),s({type:"close"})},d=()=>{s({type:"open"}),g()},m=t=>s({type:"message",value:t.data}),p=()=>{e==null||e.removeEventListener("message",m),e==null||e.removeEventListener("open",d),e==null||e.removeEventListener("close",v),e==null||e.removeEventListener("error",u),e=new WebSocket("wss://"  + location.host + "/websocket"),e.addEventListener("message",m),e.addEventListener("open",d),e.addEventListener("close",v),e.addEventListener("error",u)};self.onmessage=t=>{const{type:o,value:C}=JSON.parse(t.data);switch(o){case"initWS":{n=0,p();break}case"message":{if((e==null?void 0:e.readyState)!==1)return;c(C);break}}}})();
