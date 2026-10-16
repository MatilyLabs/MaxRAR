export interface Campaign {id:string;title:string;description:string;button:string;url:string;icon:'app'|'scan'|'archive';startsAt:number|null;endsAt:number|null;}
export interface PromotionFeed {schemaVersion:1;enabled:boolean;validUntil:number;rotationSeconds:number;campaigns:Campaign[];}
export function activeCampaigns(feed:PromotionFeed|null,dismissed:string[],now:number){
 if(!feed?.enabled||feed.validUntil<=now)return [];
 return feed.campaigns.filter(c=>!dismissed.includes(c.id)&&(c.startsAt===null||now>=c.startsAt)&&(c.endsAt===null||now<c.endsAt));
}
export function readDismissed(value:string|null):string[]{
 try{const ids:unknown=JSON.parse(value||'[]');return Array.isArray(ids)?ids.filter((id):id is string=>typeof id==='string'&&id.length<=64).slice(-100):[];}catch{return [];}
}
