// Use one-level GPU bilinear sampling rather than Canvas2D's implementation-
// dependent minification filter. Match desktop encoded-byte premultiplication.
let linearGraphics=null;
function linearSprite(x,y,w,h,width,height,pixels,straight=false,target=context){
 if(w<=0||h<=0)return;
 if(!linearGraphics){
  const surface=new OffscreenCanvas(640,480),g=surface.getContext('webgl2',{alpha:true,depth:false,antialias:false,premultipliedAlpha:true,preserveDrawingBuffer:true});
  if(!g)throw Error('Filtered game images require WebGL 2');
  const compile=(type,source)=>{const shader=g.createShader(type);g.shaderSource(shader,source);g.compileShader(shader);if(!g.getShaderParameter(shader,g.COMPILE_STATUS))throw Error('Filtered image shader failed');return shader;};
  const vertex=compile(g.VERTEX_SHADER,`#version 300 es
  uniform vec4 bounds;uniform vec4 coverage;out vec2 uv;
  void main(){vec2 p[6]=vec2[6](vec2(0,0),vec2(1,0),vec2(0,1),vec2(0,1),vec2(1,0),vec2(1,1));vec2 at=coverage.xy+p[gl_VertexID]*coverage.zw;uv=bounds.xy+p[gl_VertexID]*bounds.zw;gl_Position=vec4(at.x/320.0-1.0,1.0-at.y/240.0,0,1);}`);
  const fragment=compile(g.FRAGMENT_SHADER,`#version 300 es
  precision highp float;uniform sampler2D image;uniform bool straightAlpha;in vec2 uv;out vec4 result;
  void main(){result=texture(image,uv);if(straightAlpha)result.rgb*=result.a;}`);
  const program=g.createProgram();g.attachShader(program,vertex);g.attachShader(program,fragment);g.linkProgram(program);g.deleteShader(vertex);g.deleteShader(fragment);
  if(!g.getProgramParameter(program,g.LINK_STATUS))throw Error('Filtered image program failed');
  const image=g.createTexture();g.bindTexture(g.TEXTURE_2D,image);g.texParameteri(g.TEXTURE_2D,g.TEXTURE_MIN_FILTER,g.LINEAR);g.texParameteri(g.TEXTURE_2D,g.TEXTURE_MAG_FILTER,g.LINEAR);g.texParameteri(g.TEXTURE_2D,g.TEXTURE_WRAP_S,g.CLAMP_TO_EDGE);g.texParameteri(g.TEXTURE_2D,g.TEXTURE_WRAP_T,g.CLAMP_TO_EDGE);g.bindVertexArray(g.createVertexArray());
  linearGraphics={surface,g,program,image,bounds:g.getUniformLocation(program,'bounds')};
 }
 const {surface,g,program,image,bounds}=linearGraphics;if(g.isContextLost())throw Error('Filtered image context unavailable');
 const premultiplied=new Uint8Array(pixels.length);
 for(let i=0;i<pixels.length;i+=4){const a=pixels[i+3];for(let c=0;c<3;c++)premultiplied[i+c]=straight?pixels[i+c]:Math.floor((pixels[i+c]*a+127)/255);premultiplied[i+3]=a;}
 // Blend before framebuffer quantization, matching the desktop GPU. A
 // transparent intermediate rounds sampled color and alpha before compositing.
 g.viewport(0,0,640,480);g.useProgram(program);g.activeTexture(g.TEXTURE0);g.bindTexture(g.TEXTURE_2D,image);g.uniform1i(g.getUniformLocation(program,'image'),0);
 g.uniform1i(g.getUniformLocation(program,'straightAlpha'),0);g.disable(g.BLEND);g.texImage2D(g.TEXTURE_2D,0,g.RGBA8,g.RGBA,g.UNSIGNED_BYTE,target.canvas);g.uniform4f(bounds,0,0,1,1);g.uniform4f(g.getUniformLocation(program,'coverage'),0,0,640,480);g.drawArrays(g.TRIANGLES,0,6);
 g.uniform1i(g.getUniformLocation(program,'straightAlpha'),straight?1:0);g.enable(g.BLEND);g.blendFunc(g.ONE,g.ONE_MINUS_SRC_ALPHA);g.texImage2D(g.TEXTURE_2D,0,g.RGBA8,width,height,0,g.RGBA,g.UNSIGNED_BYTE,premultiplied);const left=Math.ceil(x-.5),top=Math.floor(y-.5)+1,right=Math.ceil(x+w-.5),bottom=Math.floor(y+h-.5)+1;g.uniform4f(bounds,(left-x)/w,(top-y)/h,(right-left)/w,(bottom-top)/h);g.uniform4f(g.getUniformLocation(program,'coverage'),left,top,right-left,bottom-top);g.drawArrays(g.TRIANGLES,0,6);
 target.imageSmoothingEnabled=false;target.drawImage(surface,0,0);
}
function render(frame,target=context,publish=true){
 // Every tick is a fresh frame, matching the native player's black backdrop.
 const background=frame.background??0x10171dff;
 if((background&255)!==255){target.fillStyle='#000000';target.fillRect(0,0,640,480);}
 target.fillStyle=color(background);target.fillRect(0,0,640,480);
 target.save();
 try {for(const entry of frame.commands){const [kind,x,y,a,b,rgba]=entry;if(kind==='clip'||kind==='clip-reset'){target.restore();target.save();if(kind==='clip'){const left=Math.round(x),top=Math.round(y),right=Math.round(x+a),bottom=Math.round(y+b);target.beginPath();target.rect(left,top,right-left,bottom-top);target.clip();}}else if(kind==='shader'){shaderFrame(x,1,1,new Uint8Array(4),target);}else if(kind==='shader-image'){shaderFrame(x,y,a,b,target);}else if(kind==='rect'){const left=Math.ceil(x-.5),top=Math.floor(y-.5)+1,right=Math.ceil(x+a-.5),bottom=Math.floor(y+b-.5)+1;target.fillStyle=color(rgba);target.fillRect(left,top,Math.max(0,right-left),Math.max(0,bottom-top));}else if(kind==='linear-image'||kind==='straight-linear-image'){const [,,,,,width,height,pixels]=entry;linearSprite(x,y,a,b,width,height,pixels,kind==='straight-linear-image',target);}else if(kind==='image'){const [,,,,,width,height,pixels]=entry;if(width===1&&height===1){const left=Math.floor(x-.5)+1,top=Math.floor(y-.5)+1,right=Math.floor(x+a-.5)+1,bottom=Math.floor(y+b-.5)+1;target.fillStyle=color((pixels[0]<<24)|(pixels[1]<<16)|(pixels[2]<<8)|pixels[3]);target.fillRect(left,top,right-left,bottom-top);}else{const surface=document.createElement('canvas');surface.width=width;surface.height=height;surface.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(pixels),width,height),0,0);target.imageSmoothingEnabled=false;target.drawImage(surface,x,y,a,b);}}else{target.fillStyle=color(b);target.beginPath();target.arc(x,y,a,0,Math.PI*2);target.fill();}}} finally {target.restore();}

 if(!data.custom_renderer&&frame.commands.length===0&&frame.background===null){const state=BigInt(frame.state);target.fillStyle='#111a2c';target.fillRect(0,0,640,480);const x=Number(state&15n),y=Number(state>>8n&255n),tx=Number(state>>16n&255n),ty=Number(state>>24n&255n);target.fillStyle='#ffd45d';target.fillRect(tx*40,ty*40,32,32);target.fillStyle='#6ee7ac';target.fillRect(x*40,y*40,32,32);}
 if(publish)publishFrameIdentity(frame);
}
