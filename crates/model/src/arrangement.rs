//! Measure clipboard operations; ranges and positions are zero-based bars.
use crate::{Automation, Song};
use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub struct MeasureClipboard {
    pub measures: Vec<[usize; 4]>,
    /// Ticks are relative to the first copied measure.
    pub automation: Vec<Automation>,
}

impl Song {
    pub fn copy_measures(&self, range: Range<usize>) -> Result<MeasureClipboard, String> {
        self.validate()?;
        if range.start >= range.end || range.end > self.arrangement.len() {
            return Err("Invalid measure range".into());
        }
        let first=(range.start*32) as u32;
        let last=(range.end*32) as u32;
        Ok(MeasureClipboard {
            measures:self.arrangement[range].to_vec(),
            automation:self.automation.iter().filter(|e|e.tick>=first && e.tick<last).map(|e|{let mut e=e.clone();e.tick-=first;e}).collect(),
        })
    }
    pub fn cut_measures(&mut self, range: Range<usize>) -> Result<MeasureClipboard, String> {
        let copied=self.copy_measures(range.clone())?;
        let mut song=self.clone();
        let count=range.end-range.start;let first=(range.start*32) as u32;let last=(range.end*32) as u32;
        song.arrangement.drain(range.clone());
        song.automation.retain(|e|e.tick<first || e.tick>=last);
        for e in &mut song.automation {if e.tick>=last {e.tick-=(count*32) as u32;}}
        let map=|p:usize| if p<=range.start {p} else if p<range.end {range.start} else {p-count};
        let end=map(song.start.saturating_add(song.length));song.start=map(song.start);song.length=end.saturating_sub(song.start).max(1);
        normalize_range(&mut song);song.validate()?;*self=song;Ok(copied)
    }
    /// Insert before `at`, or replace the same number of measures when `replace` is true.
    pub fn paste_measures(&mut self, at:usize, clip:&MeasureClipboard, replace:bool) -> Result<(),String> {
        self.validate()?;
        let count=clip.measures.len();
        if count==0 || at>self.arrangement.len() || clip.measures.iter().flatten().any(|i|*i>=32)
            || clip.automation.iter().any(|e|e.tick as usize>=count.saturating_mul(32)) {return Err("Invalid measure clipboard or position".into())}
        let end=at.checked_add(count).ok_or("Song too long")?;
        let new_len=if replace {self.arrangement.len().max(end)} else {self.arrangement.len().checked_add(count).ok_or("Song too long")?};
        if new_len>4096 {return Err("Song exceeds 4096 measures".into())}
        let mut song=self.clone();let tick=(at*32) as u32;let delta=(count*32) as u32;
        if replace {
            let old_end=end.min(song.arrangement.len());song.arrangement.splice(at..old_end,clip.measures.iter().copied());
            song.automation.retain(|e|e.tick<tick || e.tick>=tick+delta);
        } else {
            song.arrangement.splice(at..at,clip.measures.iter().copied());
            for e in &mut song.automation {if e.tick>=tick {e.tick=e.tick.checked_add(delta).ok_or("Automation exceeds song limit")?;}}
            let old_end=song.start.saturating_add(song.length);
            if at<=song.start {song.start+=count;} else if at<old_end {song.length+=count;}
        }
        for e in &clip.automation {let mut e=e.clone();e.tick=e.tick.checked_add(tick).ok_or("Automation exceeds song limit")?;song.automation.push(e);}
        song.automation.sort_by_key(|e|e.tick);normalize_range(&mut song);song.validate()?;*self=song;Ok(())
    }
}
fn normalize_range(s:&mut Song){s.start=s.start.min(s.arrangement.len().saturating_sub(1));s.length=s.length.min(s.arrangement.len().saturating_sub(s.start).max(1)).max(1);}

#[cfg(test)]
mod tests {
 use super::*;
 fn song()->Song {let mut s=Song::default();s.arrangement=(0..6).map(|i|[i,i,i,i]).collect();s.start=1;s.length=4;s.automation=vec![Automation{tick:16,track:1,controller:3,value:10},Automation{tick:64,track:2,controller:4,value:20},Automation{tick:95,track:1,controller:3,value:30},Automation{tick:96,track:0,controller:6,value:40},Automation{tick:160,track:1,controller:3,value:50}];s}
 #[test] fn copy_insert_shifts_events_and_preserves_clipboard(){let mut s=song();let clip=s.copy_measures(2..3).unwrap();assert_eq!(clip.automation.iter().map(|e|e.tick).collect::<Vec<_>>(),vec![0,31]);s.paste_measures(4,&clip,false).unwrap();assert_eq!(s.arrangement.iter().map(|a|a[0]).collect::<Vec<_>>(),vec![0,1,2,3,2,4,5]);assert_eq!(s.automation.iter().map(|e|e.tick).collect::<Vec<_>>(),vec![16,64,95,96,128,159,192]);assert_eq!((s.start,s.length),(1,5));s.validate().unwrap();}
 #[test] fn cut_moves_later_events_and_loop(){let mut s=song();let c=s.cut_measures(1..3).unwrap();assert_eq!(c.measures.len(),2);assert_eq!(c.automation.iter().map(|e|e.tick).collect::<Vec<_>>(),vec![32,63]);assert_eq!(s.automation.iter().map(|e|e.tick).collect::<Vec<_>>(),vec![16,32,96]);assert_eq!((s.start,s.length),(1,2));}
 #[test] fn replace_removes_only_target_events(){let mut s=song();let c=s.copy_measures(2..3).unwrap();s.paste_measures(3,&c,true).unwrap();assert_eq!(s.arrangement[3],[2;4]);assert_eq!(s.automation.iter().map(|e|e.tick).collect::<Vec<_>>(),vec![16,64,95,96,127,160]);assert_eq!(s.automation.last().unwrap().value,50);}
 #[test] fn invalid_operations_are_atomic(){let mut s=song();let original=s.clone();assert!(s.cut_measures(2..9).is_err());let c=MeasureClipboard{measures:vec![[32;4]],automation:vec![]};assert!(s.paste_measures(1,&c,false).is_err());assert_eq!(s,original);let c=MeasureClipboard{measures:vec![[0;4];4096],automation:vec![]};assert!(s.paste_measures(0,&c,false).is_err());assert_eq!(s,original);}
}
