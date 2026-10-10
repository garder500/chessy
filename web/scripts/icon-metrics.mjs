// Mesure les icônes forgées à 32 px dans Chromium : à quel point chaque brique se retrouve à l'œil (plus proche
// centroïde, leave-one-out), distinctivité, couverture, équilibre, contraste.
//   ICON_DUMP=/tmp/icones.json npx vitest run src/ui/forgedIcon.dump.test.ts
//   node scripts/icon-metrics.mjs /tmp/icones.json
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const input = process.argv[2];
if (!input) throw new Error("usage: node scripts/icon-metrics.mjs icones.json");
const items = JSON.parse(readFileSync(input, "utf8"));
const pairs = JSON.parse(readFileSync(input.replace(/\.json$/, "") + ".pairs.json", "utf8"));
const chrome = process.env.CHROME ?? "/opt/pw-browsers/chromium-1194/chrome-linux/chrome";
const page = `<body><pre id=out>wait</pre><canvas id=cv width=32 height=32></canvas><script>
const items=${JSON.stringify(items)}; const pairs=${JSON.stringify(pairs)}; const SZ=32;
const raster=(svg)=>new Promise(res=>{const im=new Image();im.onload=()=>{const c=document.getElementById('cv').getContext('2d',{willReadFrequently:true});c.clearRect(0,0,SZ,SZ);c.drawImage(im,0,0,SZ,SZ);res(c.getImageData(0,0,SZ,SZ).data)};im.src='data:image/svg+xml;charset=utf-8,'+encodeURIComponent(svg)});
const dist=(a,b)=>{let s=0;for(let i=0;i<a.length;i+=4){const dr=a[i]-b[i],dg=a[i+1]-b[i+1],db=a[i+2]-b[i+2];s+=dr*dr+dg*dg+db*db}return Math.sqrt(s/(a.length/4))/441.7};
let CONF=null;
function acc(vecs,labels,track){const classes=[...new Set(labels)],len=vecs[0].length,sums={},cnt={};for(const c of classes){sums[c]=new Float64Array(len);cnt[c]=0}
 vecs.forEach((v,i)=>{const s=sums[labels[i]];for(let j=0;j<len;j++)s[j]+=v[j];cnt[labels[i]]++});
 let ok=0;vecs.forEach((v,i)=>{let best=null,bd=1e18;for(const c of classes){let n=cnt[c],d=0;const s=sums[c],own=c===labels[i];if(own)n--;if(n<=0)continue;for(let j=0;j<len;j+=4){const m0=(s[j]-(own?v[j]:0))/n,m1=(s[j+1]-(own?v[j+1]:0))/n,m2=(s[j+2]-(own?v[j+2]:0))/n;d+=(v[j]-m0)**2+(v[j+1]-m1)**2+(v[j+2]-m2)**2}if(d<bd){bd=d;best=c}}if(track){const t=track['#'+labels[i]]=track['#'+labels[i]]||[0,0];t[1]++;if(best===labels[i])t[0]++}if(best===labels[i])ok++;else if(track){const key=labels[i]+'>'+best;track[key]=(track[key]||0)+1}});return ok/vecs.length}
(async()=>{const vs=[];for(const it of items)vs.push(await raster(it.svg));const m={};
 const conf={};for(const k of Object.keys(items[0].labels))m[k]=acc(vs,items.map(i=>i.labels[k]),k==='verb'?conf:null);m.perVerb=Object.entries(conf).filter(([k])=>k[0]==='#').map(([k,v])=>k.slice(1)+' '+(v[0]/v[1]).toFixed(2)).join(' ');m.confusions=Object.entries(conf).filter(([k])=>k[0]!=='#').sort((a,b)=>b[1]-a[1]).slice(0,10).map(([k,v])=>k+':'+v).join(' ');
 {const K=5,lab=items.map(i=>i.labels.verb);let ok=0;for(let i=0;i<vs.length;i++){const ds=[];for(let j=0;j<vs.length;j++)if(j!==i)ds.push([dist(vs[i],vs[j]),lab[j]]);ds.sort((a,b)=>a[0]-b[0]);const votes={};for(const [,l] of ds.slice(0,K))votes[l]=(votes[l]||0)+1;const best=Object.entries(votes).sort((a,b)=>b[1]-a[1])[0][0];if(best===lab[i])ok++}m.verb5nn=ok/vs.length}
 {// Lecteur dédié par brique : régression à noyau linéaire (ridge) sur les pixels, un contre tous, validation croisée à 2 plis.
  const n=vs.length,L=vs[0].length;const X=vs.map(v=>{const o=new Float32Array(L);for(let j=0;j<L;j++)o[j]=v[j]/255-0.5;return o});
  const dot=(a,b)=>{let s=0;for(let j=0;j<L;j++)s+=a[j]*b[j];return s};
  const solve=(A,B)=>{const m=A.length,k=B[0].length;for(let c=0;c<m;c++){let p=c;for(let r=c+1;r<m;r++)if(Math.abs(A[r][c])>Math.abs(A[p][c]))p=r;[A[c],A[p]]=[A[p],A[c]];[B[c],B[p]]=[B[p],B[c]];const d=A[c][c];for(let r=c+1;r<m;r++){const f=A[r][c]/d;if(!f)continue;for(let q=c;q<m;q++)A[r][q]-=f*A[c][q];for(let q=0;q<k;q++)B[r][q]-=f*B[c][q]}}
    const x=Array.from({length:m},()=>new Float64Array(k));for(let r=m-1;r>=0;r--){for(let q=0;q<k;q++){let s=B[r][q];for(let c=r+1;c<m;c++)s-=A[r][c]*x[c][q];x[r][q]=s/A[r][r]}}return x};
  const idx=[...Array(n).keys()];const folds=[idx.filter(i=>i%2===0),idx.filter(i=>i%2===1)];
  const lam=50;
  for(const brick of ['verb','zone','plies','mark','camp']){const lab=items.map(i=>String(i.labels[brick]));const classes=[...new Set(lab)];let ok=0;const pc={};
    for(let f=0;f<2;f++){const tr=folds[f],te=folds[1-f];const K=tr.map(i=>tr.map(j=>dot(X[i],X[j])));for(let i=0;i<tr.length;i++)K[i][i]+=lam;
      const Y=tr.map(i=>classes.map(c=>lab[i]===c?1:0));const al=solve(K,Y);
      for(const t of te){const sc=classes.map(()=>0);tr.forEach((j,a)=>{const d=dot(X[t],X[j]);for(let c=0;c<classes.length;c++)sc[c]+=d*al[a][c]});let best=0;for(let c=1;c<classes.length;c++)if(sc[c]>sc[best])best=c;const hit=classes[best]===lab[t];if(hit)ok++;const q=pc[lab[t]]=pc[lab[t]]||[0,0];q[1]++;if(hit)q[0]++}}
    m[brick+'Ridge']=ok/n;m[brick+'RidgeMin']=Math.min(...Object.values(pc).map(q=>q[0]/q[1]))}}
 const nn=[];let close=0,np=0;for(let i=0;i<vs.length;i++){let b=9;for(let j=0;j<vs.length;j++){if(i===j)continue;const d=dist(vs[i],vs[j]);if(d<b)b=d;if(j>i){np++;if(d<0.03)close++}}nn.push(b)}
 nn.sort((a,b)=>a-b);m.nnP10=nn[Math.floor(nn.length*0.1)];m.closePairs=close/np;
 const cov=vs.map(v=>{let n=0;for(let i=0;i<v.length;i+=4)if(Math.abs(v[i]-v[0])+Math.abs(v[i+1]-v[1])+Math.abs(v[i+2]-v[2])>60)n++;return n/(SZ*SZ)});
 m.cover=cov.reduce((a,b)=>a+b)/cov.length;
 const bal=vs.map(v=>{const L=[];for(let i=0;i<v.length;i+=4)L.push(0.2126*v[i]+0.7152*v[i+1]+0.0722*v[i+2]);let w=0,sx=0,sy=0;for(let p=0;p<L.length;p++){const a=Math.abs(L[p]-L[0])/255;w+=a;sx+=a*(p%SZ);sy+=a*Math.floor(p/SZ)}return Math.hypot(sx/w-(SZ-1)/2,sy/w-(SZ-1)/2)/SZ});
 m.balance=bal.reduce((a,b)=>a+b)/bal.length;
 // sensibilité : de combien l'image bouge quand une seule brique change (distance normalisée à 32 px)
 const cache=new Map();const rc=async(x)=>{if(!cache.has(x))cache.set(x,await raster(x));return cache.get(x)};const by={};for(const p of pairs){const d=dist(await rc(p.a),await raster(p.b));(by[p.brick]=by[p.brick]||[]).push(d)}
 for(const [k,v] of Object.entries(by)){v.sort((a,b)=>a-b);m['d_'+k+'_p10']=v[Math.floor(v.length*0.1)];m['d_'+k+'_med']=v[v.length>>1]}
 document.getElementById('out').textContent=JSON.stringify(m)})();
</script>`;
const dir = mkdtempSync(join(tmpdir(), "icon-metrics-"));
const file = join(dir, "metrics.html");
writeFileSync(file, page);
const dom = execFileSync(chrome, ["--headless", "--no-sandbox", "--virtual-time-budget=1500000", "--dump-dom", `file://${file}`], { encoding: "utf8", maxBuffer: 1 << 28, stdio: ["ignore", "pipe", "ignore"] });
const m = JSON.parse(dom.match(/<pre id="out">([^<]*)/)[1]);
for (const [k, v] of Object.entries(m)) console.log(k.padEnd(10), typeof v === 'number' ? v.toFixed(3) : v);
