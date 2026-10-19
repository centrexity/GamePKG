// Explicit backing-store reservations. Browser/JIT/GC overhead is not measured.
export class RecordedBudget {
 #limit;#used=0;#closed=false;#parent=null;
 constructor(limit){if(!Number.isSafeInteger(limit)||limit<1)throw Error('Invalid audio memory allowance');this.#limit=limit;}
 get limit(){return this.#limit;}
 get used(){return this.#used;}
 get closed(){return this.#closed;}
 #releaseParent(){if(this.#closed&&this.#used===0&&this.#parent){this.#parent.release();this.#parent=null;}}
 reserve(bytes){
  if(this.#closed||!Number.isSafeInteger(bytes)||bytes<1||bytes>this.#limit-this.#used)throw Error('Audio memory allowance exceeded');
  this.#used+=bytes;let released=false;
  return Object.freeze({bytes,release:()=>{if(released)return;released=true;this.#used-=bytes;this.#releaseParent();}});
 }
 partition(bytes){const lease=this.reserve(bytes);const child=new RecordedBudget(bytes);child.#parent=lease;return child;}
 close(){this.#closed=true;this.#releaseParent();}
}
