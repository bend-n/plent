"use strict";
import init, { render_schem, tags } from "/masm.js";
// import init, { render_map } from "https://apricotalliance.org/masm.js";

let tasks = [];
init().then(() => {
  window.init = true;
  tasks.forEach((t) => t());
  tasks = [];
});

const template = `<div class=schem><div class=bar><div class=background id={ID}></div><span class="typcn typcn-arrow-left"></span> <button class=squareb title=info id={ID}-info></button> <button class=squareb title=copy id={ID}-copy></button> <button class=squareb title=download id={ID}-download></button></div><span class=title id={ID}-title></span> <img id={ID}-picture onmouseenter='document.getElementById("{ID}").style.backgroundColor="#454545"' onmouseleave='document.getElementById("{ID}").style.backgroundColor="#020202"' draggable=false class=preview width=120px height=120px src=fail.png></div>`;
function b64(buf) {
  const a = new Uint8Array(buf);
  let b = "";
  for (let i = 0; i < a.byteLength; i++) {
    b += String.fromCharCode(a[i]);
  }
  return btoa(b);
}

function vis(el) {
  var rect = el.getBoundingClientRect();

  return (
    rect.top >= -100 &&
    rect.bottom <=
      (window.innerHeight || document.documentElement.clientHeight) + 200
  );
}

async function build(schems) {
  let jobs = 0;
  for (const schem of schems) {
    document
      .getElementById("grid")
      .insertAdjacentHTML("beforeend", template.replaceAll("{ID}", schem));
    let p = async function () {
      jobs += 1;
      let data = await (await fetch(`/schems/files/${schem}`)).arrayBuffer();

      let tagz;
      if (window.init) {
        tagz = tags(data);
        document.getElementById(`${schem}-title`).innerText = tagz["name"];
      } else
        tasks.push(() => {
          tagz = tags(data);
          document.getElementById(`${schem}-title`).innerText = tagz["name"];
        });

      let flag = 0;
      let pic = document.getElementById(`${schem}-picture`);
      let f = () => {
        setTimeout(() => {
          if (vis(pic) && flag == 0) {
            flag = 1;
            // SLOW (~10ms)
            if (window.init) pic.src = render_schem(data);
            else tasks.push(() => (pic.src = render_schem(data)));
            removeEventListener(pic, f);
          }
        }, 100);
      };
      addEventListener("scroll", f, false);
      f();
      let download = () => {
        Object.assign(document.createElement("a"), {
          href: `/schems/files/${schem}`,
          download: schem,
        }).click();
      };
      let copy = () => navigator.clipboard.writeText(b64(data));
      document.getElementById(`${schem}-info`).onclick = () => {
        document.getElementById("modal").className = "";
        document.getElementById("modal-name").innerText = tagz["name"];
        document.getElementById("modal-desc").innerText = tagz["description"];
        document.getElementById("modal-pic").src = pic.src;
        document.getElementById("modal-download").onclick = download;
        document.getElementById("modal-copy").onclick = copy;
        document.getElementById("body").className = "modal-open";
        fetch(`/schems/blame/${schem}`).then((x) =>
          x.text().then((x) => {
            if (x != "plent")
              document.getElementById("modal-author").innerText = x;
          })
        );
      };

      document.getElementById(`${schem}-download`).onclick = download;
      document.getElementById(`${schem}-copy`).onclick = copy;
      jobs -= 1;
    };
    if (jobs < 20) p();
    else await p();
  }
}
async function get() {
  return await (await fetch("/schems/files")).json();
}

get().then(build);
