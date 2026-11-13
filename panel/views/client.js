/* The out-of-game screen: status line, then every draft section that applies, stacked in one scroll. */
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

  /* section name -> the blocks it shows */
  var BLOCKS={picks:["secTeam","secPicks"],bans:["secBans"],build:["secBuild"],explore:["secExplore"]};

  L.register("client",{render:function(s){
    var scr=$("screen"),top=scr.scrollTop;
    var parts=[];
    if(s.role)parts.push(L.cap(s.role));if(s.days!=null&&s.days!=="")parts.push(s.days+"d");
    $("title").textContent=parts.length?parts.join(" · "):"LoL Companion";
    pending=null;  // any new state answers a tier click (accepted or not)
    renderTiers(s);

    $("secStatus").classList.toggle("hidden",!s.message);
    $("empty").classList.toggle("err",s.status==="error");
    $("msg").textContent=s.message||"";
    $("sdetail").textContent=s.detail||"";

    var any=false;
    Object.keys(BLOCKS).forEach(function(name){
      var d=s[name],on=d!=null&&d!==false;
      BLOCKS[name].forEach(function(id){$(id).classList.toggle("hidden",!on)});
      if(!on)return;
      any=true;
      if(L.sections[name])L.sections[name].render(d,s);
    });
    scr.classList.toggle("solo",!any);
    fit();  // re-derive the zoom: the explorer window is client-sized, the rest zoom with width

    var n=new Date();
    $("foot").innerHTML="Data: lolalytics · last update "+pad(n.getHours())+":"+pad(n.getMinutes())+":"+pad(n.getSeconds())+" · <b>Ctrl+X</b> show/hide";
    scr.scrollTop=top;
  }});

  $("gear").addEventListener("click",function(){L.api("open_settings")});
  /* the window's width sets the size: 300px wide is 1x (as the panel was designed). Out of game the
     window is client-sized instead and the explorer sizes to the space (explore.js), so no zoom. */
  function fit(){
    if(!$("secExplore").classList.contains("hidden"))return;
    window.setScale(document.documentElement.clientWidth/300)
  }
  window.addEventListener("resize",fit);fit();
})(window.LPH);
