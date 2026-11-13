/* Client section between games: blind-pick explorer, strength vs counter risk for one role's champions (ExploreApi). */
(function(L){
  var NS="http://www.w3.org/2000/svg",el=L.el,f=L.fmt,RETRY=30000;
  var ROLE={top:"Top",jungle:"Jungle",middle:"Mid",bottom:"Bot",support:"Support"};
  var esc=function(s){return String(s).replace(/[&<>"]/g,function(c){return{"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;"}[c]})};
  var HELP={
    "Blind score":"The overall ranking for picking this champion before the enemy laner is known: strength, plus a 50/50 mix of 'Avg matchup' (they don't counter you) and 'If countered' (they do). Higher is better.",
    "Strength":"How much more or less than an average champion in this role it wins overall, in winrate points, shrunk toward 0 when there are few games. +1.0 is about 1% more wins.",
    "If countered":"How much it loses if the enemy picks a counter: the average of its worst matchups that make up 15% of enemy picks. Counters nobody plays barely count. Closer to 0 = safer to blind.",
    "iso":"Champions on the same dashed line have the same blind score. A champion lower down can still be a fine blind if it's far enough to the right."
  };

  var box=null,ui={},champs=[],tipFor=null;
  var meta=null,role=null,tier=null,seq=0,starting=false,failedAt=0,waiting=false,last=null;
  var pv=function(){return window.pywebview&&window.pywebview.api||null};

  function build(){
    if(box)return;
    box=document.getElementById("secExplore");
    box.appendChild(el("h2","sech","Blind picks"));
    ui.roles=box.appendChild(el("div","pill xroles hidden"));
    ui.sub=box.appendChild(el("div","xsub"));
    ui.status=box.appendChild(el("div","xstatus hidden"));
    ui.svg=box.appendChild(document.createElementNS(NS,"svg"));
    ui.svg.setAttribute("class","xchart hidden");
    ui.tip=box.appendChild(el("div"));ui.tip.id="xtip";
    ui.svg.addEventListener("mousemove",hover);
    box.addEventListener("mouseleave",hideTip);
  }
  function status(t,err){ui.status.textContent=t||"";ui.status.className="xstatus"+(err?" err":"")+(t?"":" hidden")}

  /* tooltip for the portrait under the pointer, kept inside the panel */
  function hideTip(){ui.tip.style.display="none";tipFor=null}
  function tipHtml(d){
    var w3=d.opps.filter(function(o){return o.games&&o.d<0}).slice(0,3).map(function(o){return esc(o.name)+" "+f(o.d)}).join(", ");
    return "<b>"+esc(d.name)+" · blind score "+f(d.score,2)+"</b>strength "+f(d.base,2)+"<br>if countered "+f(d.tail,2)+" · avg matchup "+f(d.mean,2)+
      (d.coverage<50?"<br>few matchups ("+esc(d.coverage)+"% of picks): risk partly a typical value":"")+(w3?"<br>worst: "+w3:"");
  }
  function hover(e){
    var t=e.target.closest&&e.target.closest("[data-i]"),d=t&&champs[+t.getAttribute("data-i")];
    if(!d){hideTip();return}
    var tp=ui.tip;
    if(tipFor!==d){tipFor=d;tp.innerHTML=tipHtml(d)}
    tp.style.display="block";
    var W=document.body.clientWidth,H=document.body.clientHeight,w=tp.offsetWidth,h=tp.offsetHeight;
    tp.style.left=Math.max(2,Math.min(W-w-2,e.clientX+12))+"px";
    tp.style.top=(e.clientY+14+h<H?e.clientY+14:Math.max(2,e.clientY-h-8))+"px";
  }

  function mk(p,t,a){var e=document.createElementNS(NS,t);for(var k in a)e.setAttribute(k,a[k]);p.appendChild(e);return e}
  function txt(p,s,a){var e=mk(p,"text",a);e.textContent=s;return e}
  function hint(e,s){mk(e,"title",{}).textContent=s;return e}
  /* champion portrait, round, with a colored ring; just the ring without an icon */
  function portrait(p,url,cx,cy,d,ring,rw){
    var g=mk(p,"g",{});
    mk(g,"circle",{cx:cx,cy:cy,r:d/2+rw/2,fill:ring});
    if(url){var im=mk(g,"image",{x:cx-d/2,y:cy-d/2,width:d,height:d,"clip-path":"url(#xclip)",preserveAspectRatio:"xMidYMid slice"});
      im.setAttributeNS("http://www.w3.org/1999/xlink","href",url);im.setAttribute("href",url)}
    return g;
  }

  /* strength vs counter risk scatter, sized to the section's actual space (in design units), so the
     chart fills the wide out-of-game window instead of the whole page zooming to its width. */
  function draw(D){
    var svg=ui.svg,s=(window.LPH&&window.LPH.scale)||1;
    var r=svg.getBoundingClientRect();
    var W=Math.max(240,Math.round(r.width/s)),H=Math.max(260,Math.round(r.height/s));
    var PL=38,PR=6,PT=6,PB=28,PW=W-PL-PR,PH=H-PT-PB;
    var xs=D.map(function(d){return d.base}),ys=D.map(function(d){return d.tail});
    var x0=Math.floor(Math.min.apply(null,xs))-0.5,x1=Math.ceil(Math.max.apply(null,xs))+0.5;
    var y0=Math.floor(Math.min.apply(null,ys)*2)/2-0.25,y1=Math.min(0.25,Math.ceil(Math.max.apply(null,ys)*2)/2+0.25);
    var X=function(v){return PL+(v-x0)/(x1-x0)*PW},Y=function(v){return PT+(y1-v)/(y1-y0)*PH};
    var sx=x1-x0>8?2:1,sy=y1-y0>4?1:0.5,v;
    svg.setAttribute("viewBox","0 0 "+W+" "+H);
    svg.innerHTML='<defs><clipPath id="xclip" clipPathUnits="objectBoundingBox"><circle cx=".5" cy=".5" r=".5"/></clipPath></defs>';
    var med=ys.slice().sort(function(a,b){return a-b})[Math.floor(ys.length/2)];
    mk(svg,"rect",{x:X(0),y:PT,width:Math.max(0,X(x1)-X(0)),height:Math.max(0,Y(med)-PT),fill:"var(--q)"});
    for(v=Math.ceil(x0/sx)*sx;v<=x1;v+=sx){mk(svg,"line",{x1:X(v),x2:X(v),y1:PT,y2:PT+PH,stroke:"var(--grid)"});
      txt(svg,f(v,0),{x:X(v),y:PT+PH+11,"text-anchor":"middle","font-size":8.5})}
    for(v=Math.ceil(y0/sy)*sy;v<=y1;v+=sy){mk(svg,"line",{x1:PL,x2:PL+PW,y1:Y(v),y2:Y(v),stroke:"var(--grid)"});
      txt(svg,f(v),{x:PL-3,y:Y(v)+3,"text-anchor":"end","font-size":8.5})}
    mk(svg,"line",{x1:X(0),x2:X(0),y1:PT,y2:PT+PH,stroke:"var(--muted)","stroke-width":1.2});
    mk(svg,"line",{x1:PL,x2:PL+PW,y1:Y(med),y2:Y(med),stroke:"var(--muted)","stroke-width":1.2});
    hint(txt(svg,"strength →",{x:PL+PW,y:H-3,"text-anchor":"end","font-size":9,"class":"ax"}),HELP["Strength"]);
    hint(txt(svg,"if countered (higher = safer)",{x:10,y:PT+PH/2,transform:"rotate(-90 10 "+(PT+PH/2)+")","text-anchor":"middle","font-size":9,"class":"ax"}),HELP["If countered"]);
    [-4,-2,0,2,4].forEach(function(s){
      var pts=[];for(var b=x0;b<=x1;b+=0.05){var t=2*(s-b);if(t>=y0&&t<=y1)pts.push([X(b),Y(t)])}
      if(pts.length<2)return;
      var g=hint(mk(svg,"g",{"class":"ax"}),HELP["iso"]);
      mk(g,"polyline",{points:pts.map(function(p){return p.join(",")}).join(" "),fill:"none",stroke:"var(--dim)","stroke-dasharray":"3 3"});
    });
    txt(svg,"BLIND THESE",{x:PL+PW-4,y:PT+11,"class":"q","text-anchor":"end","font-size":8});
    txt(svg,"PICK LATE",{x:PL+PW-4,y:PT+PH-5,"class":"q","text-anchor":"end","font-size":8});
    txt(svg,"SAFE FALLBACK",{x:PL+4,y:PT+11,"class":"q","font-size":8});
    txt(svg,"AVOID BLIND",{x:PL+4,y:PT+PH-5,"class":"q","font-size":8});
    D.forEach(function(d,i){
      var cx=X(d.base),cy=Y(d.tail);
      portrait(svg,d.icon,cx,cy,18,"var(--a)",1.5);
      mk(svg,"circle",{cx:cx,cy:cy,r:10,fill:"transparent","data-i":i});
    });
  }

  /* the wide out-of-game window: zoom from its height (540 design px), not its width; below that
     width the panel is the narrow champ-select one and zooms by width like everywhere else */
  var DESIGN_H=540;
  function refit(){
    var de=document.documentElement,z=de.clientWidth>600?Math.max(1,Math.min(2,de.clientHeight/DESIGN_H)):de.clientWidth/300;
    if((window.LPH?window.LPH.scale:0)!==z)window.setScale(z)
  }
  window.addEventListener("resize",function(){
    if(!box||box.classList.contains("hidden")||!champs.length)return;
    refit();draw(champs)
  });

  function show(d){
    champs=d?d.champs:[];hideTip();
    ui.svg.classList.toggle("hidden",!d);
    ui.sub.textContent=d?"→ stronger · ↑ loses less when countered · "+d.tier_label+", "+d.days+"d":"";
    if(!d)return;
    var ln=(ROLE[role]||role||"").toLowerCase();
    ui.sub.title="Right = stronger overall. Up = loses less when the enemy "+ln+" counters it. Top right is what to blind; bottom right is strong but better picked after the enemy "+ln+" is known. "+
      d.tier_label+", last "+d.days+" days, lolalytics.\n\nBlind score: "+HELP["Blind score"];
    refit();draw(d.champs);
  }
  function pills(){
    L.clear(ui.roles);ui.roles.classList.toggle("hidden",!meta);
    if(!meta)return;
    meta.roles.forEach(function(r){
      var b=el("button",r===role?"on":null,ROLE[r]||L.cap(r));
      b.addEventListener("click",function(){if(pv()){role=r;load()}});
      ui.roles.appendChild(b);
    });
  }
  function fail(t){failedAt=Date.now();status(t,true)}
  function load(){
    var a=pv();if(!a)return;
    var my=++seq;failedAt=0;
    pills();show(null);
    status("Loading "+(ROLE[role]||role)+" stats... (first time for a role takes a few seconds)");
    a.explore_get(role,tier).then(function(d){
      if(my!==seq)return;
      if(!d||d.error){fail(d&&d.error||"No answer from the app.");return}
      if(!d.champs||!d.champs.length){status("No champions with enough games here.");return}
      status("");show(d);
    }).catch(function(e){if(my===seq)fail(String(e))});
  }
  function start(){
    starting=true;failedAt=0;status("Loading stats...");
    pv().explore_start().then(function(m){
      starting=false;meta=m;role=m.role;tier=last&&last.tier_id||m.tier;load();
    }).catch(function(e){starting=false;fail(String(e))});
  }

  L.section("explore",{render:function(d,s){
    build();last=s;refit();
    if(!pv()){
      if(!waiting){waiting=true;window.addEventListener("pywebviewready",function(){if(last&&last.explore)L.sections.explore.render(true,last)})}
      return;
    }
    if(starting)return;
    var retry=failedAt&&Date.now()-failedAt>=RETRY;
    if(!meta){if(!failedAt||retry)start();return}
    if(retry)load();
    else if(s&&s.tier_id&&s.tier_id!==tier){tier=s.tier_id;load()}
  }});

  /* screenshot harness: draw a payload without pywebview */
  window.renderExplore=function(d){
    build();box.classList.remove("hidden");
    meta={roles:L.LANE_ORDER,tier:d.tier};role=d.role;tier=d.tier;
    refit();pills();status("");show(d);
  };
})(window.LPH);
