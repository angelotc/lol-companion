/* Client section between games: blind-pick explorer, strength vs counter risk for one role's champions (ExploreApi).
   Left: the scatter (x = strength, y = if countered, the median/zero dividers split it into four quadrants).
   Right: the selected champion, then the role's best blind picks ranked by the payload's blind score. */
(function(L){
  var el=L.el,RETRY=30000;
  var ROLE={top:"Top",jungle:"Jungle",middle:"Mid",bottom:"Bot",support:"Support"};
  var esc=function(s){return String(s).replace(/[&<>"]/g,function(c){return{"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;"}[c]})};
  var HELP={
    "Blind score":"The overall ranking for picking this champion before the enemy laner is known: strength, plus a 50/50 mix of 'Avg matchup' (they don't counter you) and 'If countered' (they do). Higher is better.",
    "Strength":"How much more or less than an average champion in this role it wins overall, in winrate points, shrunk toward 0 when there are few games. +1.0 is about 1% more wins.",
    "If countered":"How much it loses if the enemy picks a counter: the average of its worst matchups that make up 15% of enemy picks. Counters nobody plays barely count. Closer to 0 = safer to blind."
  };
  /* quadrant: [verdict, colour] by (stronger than average, safer than the median) */
  var VERDICT={
    "11":["Safe to pick blind","var(--pos)"],
    "10":["Strong, but wait for their pick","var(--gold)"],
    "01":["Safe fallback, below average strength","var(--muted)"],
    "00":["Avoid picking blind","var(--neg)"]
  };

  var box=null,ui={},champs=[],ranked=[],med=0,sel=null,shownRole=null,tipFor=null;
  var meta=null,role=null,tier=null,seq=0,starting=false,failedAt=0,waiting=false,last=null;
  var pv=function(){return window.pywebview&&window.pywebview.api||null};

  /* signed number with a real minus sign */
  function f(v,d){var n=Number(v);if(!isFinite(n))return"-";var s=Math.abs(n).toFixed(d==null?1:d);
    return(+s===0?"":n>0?"+":"−")+s}
  function safe(d){return d.tail>=med}
  function strong(d){return d.base>0}
  function quad(d){return (strong(d)?"1":"0")+(safe(d)?"1":"0")}

  function build(){
    if(box)return;
    box=document.getElementById("secExplore");
    var head=box.appendChild(el("div","xhead"));
    var hl=head.appendChild(el("div","xhl"));
    hl.appendChild(el("h2","xtitle","What can you pick blind?"));
    ui.guide=hl.appendChild(el("span","xguide","Right = stronger. Up = loses less when the enemy counters it."));
    ui.roles=head.appendChild(el("div","xroles hidden"));
    ui.status=box.appendChild(el("div","xstatus hidden"));
    ui.body=box.appendChild(el("div","xbody hidden"));
    var card=ui.body.appendChild(el("div","xcard xchart"));
    ui.plot=card.appendChild(el("div","xplot"));
    var side=ui.body.appendChild(el("div","xside"));
    ui.det=side.appendChild(el("div","xcard xdet"));
    var list=side.appendChild(el("div","xcard xlist"));
    var lh=list.appendChild(el("div","xlh"));
    lh.appendChild(el("span",null,"Best blind picks")).title="Ranked by blind score. "+HELP["Blind score"];
    lh.appendChild(el("span",null,"Strength · if countered"));
    ui.rows=list.appendChild(el("div","xrows"));
    ui.tip=box.appendChild(el("div"));ui.tip.id="xtip";
    ui.plot.addEventListener("mousemove",hover);
    ui.plot.addEventListener("mouseover",hover);
    ui.plot.addEventListener("mouseleave",hideTip);
    ui.plot.addEventListener("click",function(e){var t=e.target.closest&&e.target.closest("[data-i]");if(t)pick(champs[+t.getAttribute("data-i")])});
  }
  function status(t,err){ui.status.textContent=t||"";ui.status.className="xstatus"+(err?" err":"")+(t?"":" hidden")}

  /* tooltip for the portrait under the pointer, kept inside the panel */
  function hideTip(){ui.tip.style.display="none";tipFor=null}
  function tipHtml(d){
    var w3=worst(d).map(function(o){return esc(o.name)+" "+f(o.d)}).join(", ");
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
  /* its worst matchups with data (opps come sorted by matchup delta, worst first) */
  function worst(d){return (d.opps||[]).filter(function(o){return o.games&&o.d<0}).slice(0,3)}

  function pick(d){if(!d||d.id===sel)return;sel=d.id;hideTip();draw()}
  function selected(){return champs.filter(function(d){return d.id===sel})[0]||ranked[0]}

  function abs(cls,x,y,text){
    var e=el("div",cls,text);
    if(x!=null)e.style.left=x+"%";
    if(y!=null)e.style.top=y+"%";
    return ui.plot.appendChild(e);
  }

  /* strength vs counter risk, in % of the plot box so it scales with the window; the domain comes
     from the data (padded to round numbers), the y divider is the role's median "if countered". */
  function plot(){
    var P=ui.plot,v;L.clear(P);
    var xs=champs.map(function(d){return d.base}),ys=champs.map(function(d){return d.tail});
    var x0=Math.floor(Math.min.apply(null,xs))-0.5,x1=Math.ceil(Math.max.apply(null,xs))+0.5;
    var y0=Math.floor(Math.min.apply(null,ys)*2)/2-0.25,y1=Math.min(0.25,Math.ceil(Math.max.apply(null,ys)*2)/2+0.25);
    var X=function(v){return (v-x0)/(x1-x0)*100},Y=function(v){return (y1-v)/(y1-y0)*100};
    var sx=x1-x0>8?2:1,sy=y1-y0>4?1:0.5;
    var xz=Math.max(0,Math.min(100,X(0))),ym=Y(med);

    var q=abs("xq",xz,0);q.style.right="0";q.style.height=ym+"%";
    for(v=Math.ceil(y0/sy)*sy;v<=y1+1e-9;v+=sy){
      abs("xgrid",null,Y(v));
      abs("xyt",null,Y(v),f(v));
    }
    var tx=[];
    for(v=Math.ceil(x0/sx)*sx;v<=x1+1e-9;v+=sx)tx.push(abs("xxt"+(Math.abs(v)<1e-9?" zero":""),X(v),null,Math.abs(v)<1e-9?"0 · average":f(v,0)));
    var xtl=abs("xxt ttl",null,null,"strength →");xtl.title=HELP["Strength"];
    var ytl=abs("xytl",null,null,"if countered (higher = safer) →");ytl.title=HELP["If countered"];
    if(X(0)>0&&X(0)<100)abs("xdiv v",X(0),null);
    abs("xdiv h",null,ym);
    abs("xql good",null,null,"BLIND THESE");
    abs("xql tl",null,null,"SAFE BUT WEAK");
    abs("xql br",null,null,"STRONG, PICK LATE");
    abs("xql bl",null,null,"AVOID BLIND");

    /* portraits: plain, "blind these" with a green ring, the selected one gold and on top */
    var pts=champs.map(function(d,i){
      var on=d.id===sel,good=strong(d)&&safe(d);
      var b=abs("xpt"+(good?" good":"")+(on?" on":""),X(d.base),Y(d.tail));
      b.setAttribute("data-i",i);b.setAttribute("aria-label",d.name);
      b.appendChild(L.icon(d.name,d.icon));
      return {d:d,el:b,on:on,good:good};
    });
    /* x tick labels that would run under the axis title are dropped */
    var R=P.getBoundingClientRect();
    if(R.width){
      var tl=xtl.getBoundingClientRect().left-4;
      tx.forEach(function(t){if(t.getBoundingClientRect().right>tl)t.remove()});
    }
    labels(pts,R);
  }

  /* names under the selected portrait and the "blind these" ones (best first), skipping any that
     would overlap a portrait other than its own or a label already placed (greedy) */
  function labels(pts,R){
    if(!R.width)return;
    var ox=R.left,oy=R.top,box=function(r,k){k=k||0;var w=(r.right-r.left)*k,h=(r.bottom-r.top)*k;
      return {l:r.left-ox+w,r:r.right-ox-w,t:r.top-oy+h,b:r.bottom-oy-h}};
    var hit=function(a,b){return a.l<b.r&&b.l<a.r&&a.t<b.b&&b.t<a.b};
    var rects=pts.map(function(p){return box(p.el.getBoundingClientRect(),.12)});  // slight overlap with a ring is fine
    var placed=[];
    var want=pts.map(function(p,i){return{p:p,i:i}}).filter(function(o){return o.p.on||o.p.good})
      .sort(function(a,b){return (b.p.on-a.p.on)||(b.p.d.score-a.p.d.score)});
    var clash=function(r,i){return r.l<0||r.r>R.width||r.t<0||r.b>R.height||placed.some(function(q){return hit(r,q)})||
      rects.some(function(q,j){return j!==i&&hit(r,q)})};
    want.forEach(function(o){
      var p=o.p,lab=abs("xlab"+(p.on?" on":""),parseFloat(p.el.style.left),parseFloat(p.el.style.top),p.d.name);
      var r=box(lab.getBoundingClientRect());
      if(clash(r,o.i)){  // under the portrait is taken: try above it; the selected one keeps its label anyway
        lab.classList.add("up");var up=box(lab.getBoundingClientRect());
        if(!clash(up,o.i))r=up;
        else if(p.on)lab.classList.remove("up");
        else{lab.remove();return}
      }
      placed.push(r);
    });
  }

  function detail(){
    var D=ui.det,d=selected();L.clear(D);if(!d)return;
    var v=VERDICT[quad(d)];
    var top=D.appendChild(el("div","xdt"));
    top.appendChild(L.icon(d.name,d.icon,"xdp"));
    var nm=top.appendChild(el("div","xdn"));
    nm.appendChild(el("div","xname",d.name));
    var vd=nm.appendChild(el("div","xverd",v[0]));vd.style.color=v[1];
    var tiles=D.appendChild(el("div","xtiles"));
    [["Strength",d.base,d.base>0?"pos":"neg"],["If countered",d.tail,d.tail<0?"neg":"pos"]].forEach(function(t){
      var b=tiles.appendChild(el("div","xtile"));b.title=HELP[t[0]];
      b.appendChild(el("div","cap",t[0]));
      b.appendChild(el("div","xtv "+t[2],f(t[1])));
    });
    var w=worst(d);
    if(w.length){
      var row=D.appendChild(el("div","xworst"));
      row.appendChild(el("span","xwl","Worst counters"));
      var ws=row.appendChild(el("span","xws"));
      w.forEach(function(o){
        var s=ws.appendChild(el("span","xw"));s.title=o.name+": "+f(o.d)+" in this matchup";
        s.appendChild(L.icon(o.name,o.icon));s.appendChild(el("span","xwn",o.name));
      });
    }
  }

  /* the ranked list, as many rows as fit the card */
  function list(){
    var B=ui.rows;L.clear(B);
    ranked.slice(0,12).forEach(function(d,i){
      var b=B.appendChild(el("button","xrow"+(d.id===sel?" on":"")));
      b.appendChild(el("span","xrk",String(i+1)));
      b.appendChild(L.icon(d.name,d.icon));
      b.appendChild(el("span","xrn",d.name));
      b.appendChild(el("span","xrs "+(d.base>0?"pos":"neg"),f(d.base)));
      b.appendChild(el("span","xrc "+(d.tail<0?"neg":"pos"),f(d.tail)));
      b.title="blind score "+f(d.score,2);
      b.addEventListener("click",function(){pick(d)});
    });
    var lim=B.getBoundingClientRect().bottom;
    if(lim)[].slice.call(B.children).forEach(function(r){if(r.getBoundingClientRect().bottom>lim+.5)r.remove()});
  }

  function draw(){
    if(!champs.length)return;
    plot();detail();list();
  }

  window.addEventListener("resize",function(){
    if(!box||box.classList.contains("hidden")||!champs.length)return;
    requestAnimationFrame(draw);  // after client.js has re-zoomed the page
  });

  function show(d){
    champs=d?d.champs:[];hideTip();
    ui.body.classList.toggle("hidden",!d);
    if(!d)return;
    ranked=champs.slice().sort(function(a,b){return b.score-a.score});
    var ys=champs.map(function(c){return c.tail}).sort(function(a,b){return a-b});
    med=ys[Math.floor(ys.length/2)];
    /* a new role starts on its best blind pick; the same role (new tier) keeps the selection if it's there */
    if(d.role!==shownRole||!champs.some(function(c){return c.id===sel}))sel=ranked[0].id;
    shownRole=d.role;
    var ln=(ROLE[d.role]||d.role||"").toLowerCase();
    ui.guide.title="Right = stronger overall. Up = loses less when the enemy "+ln+" counters it. Top right is what to blind; bottom right is strong but better picked after the enemy "+ln+" is known. "+
      (d.tier_label?d.tier_label+", ":"")+"last "+d.days+" days, lolalytics.\n\nBlind score: "+HELP["Blind score"];
    draw();
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
    build();last=s;
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
    pills();status("");show(d);
  };
})(window.LPH);
