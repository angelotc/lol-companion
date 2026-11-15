/* The panel window's screen: the title bar, then in champ select the draft bar (views/draftbar.js)
   and one view at a time under the step row (1 bans, 2 blind pick, 3 best picks, 4 build), or
   between games the explorer; a status line when there's nothing else to show. */
(function(L){
  var $=L.$;
  function pad(n){return(n<10?"0":"")+n}

  var pending=null;  // tier id clicked, until the next state answers it
  function renderTiers(s){
    var box=$("tiers");
    $("tiersBox").classList.toggle("hidden",!(s.tiers&&s.tiers.length>1));
    L.clear(box);
    (s.tiers||[]).forEach(function(t){
      var b=L.el("button",(pending||s.tier_id)===t.id?"on":null,t.label);
      b.disabled=!!pending;
      b.addEventListener("click",function(){
        if(pending||t.id===s.tier_id)return;
        pending=t.id;renderTiers(s);
        if(!L.api("set_tier",t.id)){pending=null;renderTiers(s)}
      });
      box.appendChild(b);
    });
  }

  /* champ select's views, in draft order; each shows while it applies */
  var TABS=[
    {id:"bans",n:"1",label:"Bans",sec:"secBans",tip:"Enemy champions worth banning (ban phase)",
     has:function(s){return !!s.bans}},
    {id:"blind",n:"2",label:"Blind pick",sec:"secPicks",tip:"Picks that hold up whoever your lane opponent picks (while they're unknown)",
     has:function(s){var p=s.picks;return !!(p&&p.blind&&p.blind.length&&!p.locked)}},
    {id:"best",n:"3",label:"Best picks",sec:"secPicks",tip:"Picks ranked against the enemies picked so far and with your team",
     has:function(s){return !!s.picks}},
    {id:"build",n:"4",label:"Build",off:"Build · after lock-in",sec:"secBuild",tip:"Runes, spells, skills and items for your lock-in, vs your lane (after lock-in)",
     has:function(s){return !!s.build}}
  ];
  var AUTO=["build","bans","blind","best"];  // where the draft is: the first of these that applies
  var chosen=null,lastAuto=null,lastActive=null,last=null;

  function tabs(s){
    var on=TABS.filter(function(t){return t.has(s)}),ids=on.map(function(t){return t.id});
    var auto=AUTO.filter(function(id){return ids.indexOf(id)>=0})[0]||null;
    if(auto!==lastAuto){lastAuto=auto;chosen=null}  // the draft moved on: follow it again
    if(chosen&&ids.indexOf(chosen)<0)chosen=null;
    return{on:on,auto:auto,active:chosen||auto};
  }
  /* every step, in draft order: the ones that don't apply yet (or any more) are disabled; a badge
     marks where the draft is, the underline the one you're looking at */
  function strip(t){
    var box=$("steps");L.clear(box);$("stepsRow").classList.toggle("hidden",!t.on.length);
    TABS.forEach(function(x){
      var has=t.on.indexOf(x)>=0;
      var b=L.el("button",x.id===t.active?"on":null);
      b.appendChild(L.el("span","n",x.n));
      b.appendChild(document.createTextNode(!has&&x.off?x.off:x.label));
      if(x.id===t.auto)b.appendChild(L.el("span","badge","DRAFT IS HERE"));
      if(x.id===t.active)b.setAttribute("aria-current","page");
      b.disabled=!has;
      b.title=x.tip+(!has?"":x.id===t.auto?". Where the draft is now.":". Click to look; the next phase brings you back.");
      b.addEventListener("click",function(){chosen=x.id===t.auto?null:x.id;sections(last)});
      box.appendChild(b);
    });
  }
  function sections(s){
    var t=tabs(s),view=t.active;
    var sec=view?TABS.filter(function(x){return x.id===view})[0].sec:null;
    strip(t);
    $("secTeam").classList.toggle("hidden",!s.picks);
    ["secBans","secPicks","secBuild"].forEach(function(id){$(id).classList.toggle("hidden",id!==sec)});
    if(s.picks)L.sections.draftbar.render(s.picks,s);  // the draft bar and the step row's right side, always
    if(s.picks&&(view==="blind"||view==="best"))L.sections.picks.render(s.picks,s,view);
    if(view==="bans")L.sections.bans.render(s.bans,s);
    if(view==="build")L.sections.build.render(s.build,s);
    $("secExplore").classList.toggle("hidden",!s.explore);
    if(s.explore)L.sections.explore.render(s.explore,s);
    $("screen").classList.toggle("solo",!(t.on.length||s.explore));
    if(view!==lastActive){lastActive=view;$("screen").scrollTop=0}
  }

  L.register("client",{render:function(s){
    var scr=$("screen"),top=scr.scrollTop;
    last=s;
    /* title bar: role (the explorer picks its own) · tier · window */
    var parts=[];
    if(s.role&&!(s.explore&&!s.picks))parts.push(L.cap(s.role));
    if(s.tier)parts.push(s.tier);
    if(s.days!=null&&s.days!=="")parts.push("last "+s.days+" days");
    $("title").textContent=parts.join(" · ");
    pending=null;  // any new state answers a tier click (accepted or not)
    renderTiers(s);

    /* status: in the title bar beside the explorer, else a line above the content (centered when alone) */
    var inBar=!!(s.message&&s.explore);
    $("tbStatus").classList.toggle("hidden",!inBar);
    $("tbStatus").classList.toggle("err",s.status==="error");
    $("tbMsg").textContent=inBar?s.message+(s.detail?" · "+s.detail:""):"";
    $("tbStatus").title=inBar?$("tbMsg").textContent:"";
    $("foot").classList.toggle("hidden",inBar);
    $("secStatus").classList.toggle("hidden",!s.message||inBar);
    $("empty").classList.toggle("err",s.status==="error");
    $("msg").textContent=s.message||"";
    $("sdetail").textContent=s.detail||"";

    var view=lastActive;
    sections(s);
    fit();

    var n=new Date();
    $("foot").textContent="lolalytics · updated "+pad(n.getHours())+":"+pad(n.getMinutes());
    $("foot").title="Stats from lolalytics (cached 24h); this screen last updated at "+pad(n.getHours())+":"+pad(n.getMinutes())+":"+pad(n.getSeconds());
    if(lastActive===view)scr.scrollTop=top;  // same view: keep your place
  }});

  $("gear").addEventListener("click",function(){L.api("open_settings")});
  /* one page for every stage: designed at 960x540 (the window keeps that 16:9 shape) and zoomed to
     fill it, by the smaller side when maximized or snapped */
  var DESIGN=[960,540];
  function fit(){
    var de=document.documentElement,z=Math.min(de.clientWidth/DESIGN[0],de.clientHeight/DESIGN[1]);
    if(z>0&&z!==L.scale)window.setScale(z);
  }
  window.addEventListener("resize",fit);fit();
})(window.LPH);
