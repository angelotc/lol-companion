/* The panel window's screen: status line, then in champ select the team strip and one view at a
   time under a tab strip (1a bans, 1b blind pick, 1c best picks, 1d build), or between games the explorer. */
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
    {id:"bans",n:"1a",label:"Bans",sec:"secBans",tip:"Enemy champions worth banning (ban phase)",
     has:function(s){return !!s.bans}},
    {id:"blind",n:"1b",label:"Blind pick",sec:"secPicks",tip:"Picks that hold up whoever your lane opponent picks (while they're unknown)",
     has:function(s){var p=s.picks;return !!(p&&p.blind&&p.blind.length&&!p.locked)}},
    {id:"best",n:"1c",label:"Best picks",sec:"secPicks",tip:"Picks ranked against the enemies picked so far and with your team",
     has:function(s){return !!s.picks}},
    {id:"build",n:"1d",label:"Build",sec:"secBuild",tip:"Runes, spells, skills and items for your lock-in, vs your lane (after lock-in)",
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
  function strip(t){
    var box=$("steps");L.clear(box);box.classList.toggle("hidden",!t.on.length);
    t.on.forEach(function(x){
      var b=L.el("button",(x.id===t.active?"on":"")+(x.id===t.auto&&t.active!==t.auto?" now":""));
      b.appendChild(L.el("b",null,x.n));b.appendChild(document.createTextNode(x.label));
      b.title=x.tip+(x.id===t.auto?". Where the draft is now.":". Click to look; the next phase brings you back.");
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
    if(s.picks)L.sections.picks.render(s.picks,s,view==="blind"?"blind":"best");  // the team strip, always
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
    var parts=[];
    if(s.role)parts.push(L.cap(s.role));if(s.days!=null&&s.days!=="")parts.push(s.days+"d");
    $("title").textContent=parts.length?parts.join(" · "):"LoL Companion";
    pending=null;  // any new state answers a tier click (accepted or not)
    renderTiers(s);

    $("secStatus").classList.toggle("hidden",!s.message);
    $("empty").classList.toggle("err",s.status==="error");
    $("msg").textContent=s.message||"";
    $("sdetail").textContent=s.detail||"";

    var view=lastActive;
    sections(s);
    fit();

    var n=new Date();
    $("foot").innerHTML="Data: lolalytics · last update "+pad(n.getHours())+":"+pad(n.getMinutes())+":"+pad(n.getSeconds())+" · <b>Ctrl+X</b> show/hide";
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
