/* Blind-pick explorer: strength vs counter risk for one role's champions (data from ExploreApi.get). */
(function(){
  var NS="http://www.w3.org/2000/svg",$=function(id){return document.getElementById(id)};
  var ROLE={top:"Top",jungle:"Jungle",middle:"Mid",bottom:"Bot",support:"Support"};
  var state={role:"support",tier:"master_plus",data:null};
  var f=function(v,d){d=d==null?1:d;return(v>0?"+":"")+Number(v).toFixed(d)};
  var esc=function(s){return String(s).replace(/[&<>"]/g,function(c){return{"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;"}[c]})};

  var HELP={
    "Blind score":"The overall ranking for picking this champion before the enemy laner is known: strength, plus a 50/50 mix of 'Avg matchup' (they don't counter you) and 'If countered' (they do). Higher is better.",
    "Strength":"How much more or less than an average champion in this role it wins overall, in winrate points, shrunk toward 0 when there are few games. +1.0 is about 1% more wins.",
    "If countered":"How much it loses if the enemy picks a counter: the average of its worst matchups that make up 15% of enemy picks. Counters nobody plays barely count. Closer to 0 = safer to blind.",
    "iso":"Champions on the same dashed line have the same blind score. A champion lower down can still be a fine blind if it's far enough to the right."
  };

  /* tooltips: anything with data-help shows its explanation; chart marks set data-tip html */
  var tip=$("tip");
  function show(e,html){tip.style.display="block";tip.innerHTML=html;
    tip.style.left=Math.min(e.clientX+14,innerWidth-tip.offsetWidth-8)+"px";
    tip.style.top=Math.min(e.clientY+14,innerHeight-tip.offsetHeight-8)+"px"}
  function onMove(e){
    var t=e.target.closest&&e.target.closest("[data-help],[data-tip]");
    if(!t){tip.style.display="none";return}
    if(t.dataset.tip)show(e,t.dataset.tip);
    else show(e,"<b>"+esc(t.dataset.title||t.dataset.help)+"</b>"+esc(HELP[t.dataset.help]||""));
  }
  document.addEventListener("mousemove",onMove);
  document.addEventListener("mouseleave",function(){tip.style.display="none"});

  function mk(svg,t,a){var e=document.createElementNS(NS,t);for(var k in a)e.setAttribute(k,a[k]);svg.appendChild(e);return e}
  function txt(svg,s,a){var e=mk(svg,"text",a);e.textContent=s;return e}
  function infoDot(svg,x,y,key){
    var g=mk(svg,"g",{"data-help":key});
    mk(g,"circle",{cx:x,cy:y,r:6.5,fill:"var(--bg)",stroke:"var(--muted)"});
    txt(g,"i",{x:x,y:y+3.5,"text-anchor":"middle","font-size":9,"font-weight":700});
    return g;
  }
  function size(svg,W,H){svg.setAttribute("viewBox","0 0 "+W+" "+H);svg.setAttribute("width",W);svg.setAttribute("height",H);
    svg.innerHTML='<defs><clipPath id="'+svg.id+'-c" clipPathUnits="objectBoundingBox"><circle cx=".5" cy=".5" r=".5"/></clipPath></defs>'}
  /* champion portrait, round, with a colored ring; falls back to a dot without an icon */
  function portrait(svg,url,cx,cy,d,ring,rw,attrs){
    var g=mk(svg,"g",attrs||{});
    mk(g,"circle",{cx:cx,cy:cy,r:d/2+rw/2,fill:ring});
    if(url){var im=mk(g,"image",{x:cx-d/2,y:cy-d/2,width:d,height:d,"clip-path":"url(#"+svg.id+"-c)",preserveAspectRatio:"xMidYMid slice"});
      im.setAttributeNS("http://www.w3.org/1999/xlink","href",url);im.setAttribute("href",url)}
    return g;
  }

  var lane=function(){return ROLE[state.role].toLowerCase()};

  /* strength vs counter risk scatter */
  function drawScatter(){
    var D=state.data.champs,svg=$("scatter"),W=1000,H=660,L=64,R=24,T=20,B=48,PW=W-L-R,PH=H-T-B;
    var xs=D.map(function(d){return d.base}),ys=D.map(function(d){return d.tail});
    var x0=Math.floor(Math.min.apply(null,xs))-0.5,x1=Math.ceil(Math.max.apply(null,xs))+0.5;
    var y0=Math.floor(Math.min.apply(null,ys)*2)/2-0.25,y1=Math.min(0.25,Math.ceil(Math.max.apply(null,ys)*2)/2+0.25);
    var X=function(v){return L+(v-x0)/(x1-x0)*PW},Y=function(v){return T+(y1-v)/(y1-y0)*PH};
    size(svg,W,H);
    var med=ys.slice().sort(function(a,b){return a-b})[Math.floor(ys.length/2)];
    mk(svg,"rect",{x:X(0),y:T,width:X(x1)-X(0),height:Y(med)-T,fill:"var(--q)"});
    for(var v=Math.ceil(x0);v<=x1;v++){mk(svg,"line",{x1:X(v),x2:X(v),y1:T,y2:T+PH,stroke:"var(--grid)"});txt(svg,f(v,0),{x:X(v),y:T+PH+16,"text-anchor":"middle"})}
    for(v=Math.ceil(y0*2)/2;v<=y1;v+=0.5){mk(svg,"line",{x1:L,x2:L+PW,y1:Y(v),y2:Y(v),stroke:"var(--grid)"});txt(svg,f(v),{x:L-8,y:Y(v)+4,"text-anchor":"end"})}
    mk(svg,"line",{x1:X(0),x2:X(0),y1:T,y2:T+PH,stroke:"var(--muted)","stroke-width":1.5});
    mk(svg,"line",{x1:L,x2:L+PW,y1:Y(med),y2:Y(med),stroke:"var(--muted)","stroke-width":1.5});
    txt(svg,"strength (winrate pts vs average) →",{x:L+PW-20,y:H-8,"text-anchor":"end"});infoDot(svg,L+PW-8,H-12,"Strength");
    txt(svg,"if countered (winrate pts), higher is safer →",{x:14,y:T+PH,transform:"rotate(-90 14 "+(T+PH)+")"});
    infoDot(svg,10,T+PH-300,"If countered");
    [-4,-2,0,2,4].forEach(function(s){
      var pts=[];for(var b=x0;b<=x1;b+=0.05){var t=2*(s-b);if(t>=y0&&t<=y1)pts.push([X(b),Y(t)])}
      if(pts.length<2)return;
      mk(svg,"polyline",{points:pts.map(function(p){return p.join(",")}).join(" "),fill:"none",stroke:"var(--dim)","stroke-dasharray":"4 4"});
      var g=txt(svg,"score "+f(s,0),{x:pts[0][0]+4,y:pts[0][1]+12,"class":"iso","data-help":"iso","data-title":"Blind score lines"});
    });
    txt(svg,"Blind these",{x:L+PW-8,y:T+44,"class":"quad","text-anchor":"end"});
    txt(svg,"Strong, pick late",{x:L+PW-8,y:T+PH-10,"class":"quad","text-anchor":"end"});
    txt(svg,"Safe fallback",{x:L+12,y:T+44,"class":"quad"});
    txt(svg,"Avoid blind",{x:L+12,y:T+PH-10,"class":"quad"});
    var boxes=[];
    D.forEach(function(d){
      var cx=X(d.base),cy=Y(d.tail);
      portrait(svg,d.icon,cx,cy,26,"var(--a)",2);
      var w=d.name.length*6.3,cand=[[cx+17,cy+4,"start"],[cx-17,cy+4,"end"],[cx,cy+27,"middle"],[cx,cy-19,"middle"]];
      for(var i=0;i<cand.length;i++){var lx=cand[i][0],ly=cand[i][1],an=cand[i][2],bx=an==="start"?lx:an==="end"?lx-w:lx-w/2,bb={x:bx,y:ly-10,w:w,h:12};
        if(bx<L||bx+w>L+PW)continue;
        if(!boxes.some(function(o){return bb.x<o.x+o.w&&bb.x+bb.w>o.x&&bb.y<o.y+o.h&&bb.y+bb.h>o.y})){boxes.push(bb);txt(svg,d.name,{x:lx,y:ly,"class":"lab","text-anchor":an});break}}
      var w3=d.opps.filter(function(o){return o.games&&o.d<0}).slice(0,3).map(function(o){return esc(o.name)+" "+f(o.d)}).join(", ");
      mk(svg,"circle",{cx:cx,cy:cy,r:14,fill:"transparent","data-tip":"<b>"+esc(d.name)+" · blind score "+f(d.score,2)+"</b>strength "+f(d.base,2)+"<br>if countered "+f(d.tail,2)+" · avg matchup "+f(d.mean,2)+
        (d.coverage<50?"<br>few matchups ("+d.coverage+"% of picks): risk partly a typical value":"")+(w3?"<br>worst: "+w3:"")});
    });
  }

  function render(){
    $("chart").classList.toggle("hidden",!state.data);
    if(!state.data)return;
    $("sub").textContent="Right = stronger overall. Up = loses less when the enemy "+ROLE[state.role].toLowerCase()+" counters it. "+
      "Top right is what to blind; bottom right is strong but better picked after the enemy "+ROLE[state.role].toLowerCase()+" is known. "+
      state.data.tier_label+", last "+state.data.days+" days, lolalytics.";
    drawScatter();
  }

  function pills(id,items,cur,pick){
    var box=$(id);box.innerHTML="";
    items.forEach(function(it){var b=document.createElement("button");b.textContent=it.label;b.className=it.id===cur?"on":"";
      b.addEventListener("click",function(){pick(it.id)});box.appendChild(b)});
  }
  var meta=null,seq=0;
  function controls(){
    pills("roles",meta.roles.map(function(r){return{id:r,label:ROLE[r]||r}}),state.role,function(r){state.role=r;load()});
    $("tiers").classList.toggle("hidden",meta.tiers.length<2);
    pills("tiers",meta.tiers,state.tier,function(t){state.tier=t;load()});
  }
  function load(){
    controls();
    var my=++seq,st=$("status");st.className="status";st.textContent="Loading "+ROLE[state.role]+" stats... (first time for a role takes a few seconds)";
    state.data=null;render();$("sub").textContent="";
    window.pywebview.api.get(state.role,state.tier).then(function(d){
      if(my!==seq)return;
      if(d.error||!d.champs||!d.champs.length){st.className="status err";st.textContent=d.error||"No champions with enough games here.";return}
      st.textContent="";st.className="status hidden";show_(d);
    }).catch(function(e){if(my===seq){st.className="status err";st.textContent=String(e)}});
  }
  function show_(d){state.data=d;render()}
  function init(){
    window.pywebview.api.start().then(function(m){meta=m;state.role=m.role;state.tier=m.tier;load()});
  }
  /* screenshot harness: render a payload without pywebview */
  window.renderExplore=function(d){meta={roles:["top","jungle","middle","bottom","support"],tiers:[{id:d.tier,label:d.tier_label}]};
    state.role=d.role;state.tier=d.tier;controls();$("status").className="status hidden";show_(d)};
  if(window.pywebview&&window.pywebview.api)init();
  else window.addEventListener("pywebviewready",init);
})();
