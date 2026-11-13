/* Shared helpers and the screen router. Python calls updateState(state); state.view picks the page's screen. */
(function(){
  var $=function(id){return document.getElementById(id)};
  var LANES={top:"top",jungle:"jg",jg:"jg",middle:"mid",mid:"mid",bottom:"bot",bot:"bot",adc:"bot",support:"sup",utility:"sup",sup:"sup"};
  function el(tag,cls,text){var e=document.createElement(tag);if(cls)e.className=cls;if(text!=null)e.textContent=text;return e}
  function lane(l){return l?(LANES[String(l).toLowerCase()]||String(l).slice(0,3)):""}
  function icon(name,url,cls){
    var w=el("span","ico"+(cls?" "+cls:""));
    var fb=el("span","fb",String(name||"?").replace(/[^A-Za-z0-9]/g,"").slice(0,2));
    w.appendChild(fb);
    if(url){var im=document.createElement("img");im.alt="";im.loading="lazy";
      im.onerror=function(){im.remove()};
      im.onload=function(){fb.remove()};
      im.src=url;w.appendChild(im);}
    return w;
  }
  function fmt(v,d){var n=Number(v);if(!isFinite(n))return"-";return(n>0?"+":"")+n.toFixed(d==null?1:d)}
  function cls(v){return Number(v)>=0?"pos":"neg"}
  function clear(n){while(n.firstChild)n.removeChild(n.firstChild)}
  function short(n){n=String(n||"");return n.length>8?n.slice(0,7)+"…":n}
  function cap(s){s=String(s||"");return s.charAt(0).toUpperCase()+s.slice(1)}
  /* call window.pywebview.api.<name>(...); false when not running inside the app */
  function api(name){
    var args=[].slice.call(arguments,1);
    try{var a=window.pywebview&&window.pywebview.api;if(a&&a[name]){a[name].apply(a,args);return true}}catch(e){}
    return false;
  }

  /* screens: one per page ("client" or "game"); sections: parts of the client screen */
  var screens={},sections={};
  function register(name,screen){screens[name]=screen}
  function section(name,view){sections[name]=view}

  window.updateState=function(s){
    s=s||{};
    if(tip){tip.remove();tip=null}  // its element is about to be redrawn
    if(screens[s.view])screens[s.view].render(s);
  };
  window.setScale=function(v){v=parseFloat(v);if(isFinite(v)&&v>0){document.documentElement.style.setProperty("--s",v);if(window.LPH)window.LPH.scale=v}};
  window.setMode=function(m){document.body.dataset.mode=m||""};
  /* In game the panel never takes focus, and native tooltips may need it: draw our own from title */
  var tip=null;
  document.addEventListener("mouseover",function(e){
    if(document.body.dataset.mode!=="ingame")return;
    var t=e.target.closest&&e.target.closest("[title],[data-tip]");
    if(t&&t.title){t.dataset.tip=t.title;t.removeAttribute("title")}
    if(!t){if(tip){tip.remove();tip=null}return}
    if(!tip){tip=el("div",null);tip.id="tip";document.body.appendChild(tip)}
    var W=document.body.clientWidth,H=document.body.clientHeight;  // the panel, not the window
    tip.style.maxWidth=(W-4)+"px";tip.textContent=t.dataset.tip;
    var r=t.getBoundingClientRect(),h=tip.offsetHeight,wd=tip.offsetWidth;
    tip.style.left=Math.max(2,Math.min(W-wd-2,r.left))+"px";
    tip.style.top=(r.bottom+h+4<H?r.bottom+3:r.top-h-3)+"px";
  });
  document.documentElement.addEventListener("mouseleave",function(){if(tip){tip.remove();tip=null}});

  /* The enemy team's damage split: a stacked bar (physical / magic / true) with the % as text;
     hover for each enemy's own split. */
  function damage(d){
    var box=el("div","tdmg");
    var bar=el("div","tdmg-bar");
    [["physical","d-ad"],["magic","d-ap"],["true","d-tr"]].forEach(function(t){
      if(d[t[0]]>0){var seg=el("span",t[1]);seg.style.width=d[t[0]]+"%";bar.appendChild(seg)}
    });
    var txt=el("div","tdmg-t");
    txt.appendChild(el("span",null,"Their damage"));
    txt.appendChild(el("b","d-ad",d.physical+"% AD"));
    txt.appendChild(el("b","d-ap",d.magic+"% AP"));
    if(d["true"]>=3)txt.appendChild(el("b","d-tr",d["true"]+"% true"));
    box.appendChild(txt);box.appendChild(bar);
    box.title=(d.enemies||[]).map(function(e){
      return e.name+": "+e.physical+"% AD, "+e.magic+"% AP"+(e["true"]?", "+e["true"]+"% true":"");
    }).join("\n");
    return box;
  }

  window.LPH={$:$,el:el,damage:damage,lane:lane,icon:icon,fmt:fmt,cls:cls,clear:clear,short:short,cap:cap,api:api,
    register:register,section:section,sections:sections,scale:1,  // current page zoom (setScale)
    LANE_ORDER:["top","jungle","middle","bottom","support"]};
})();
