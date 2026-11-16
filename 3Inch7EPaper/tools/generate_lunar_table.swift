import Foundation
let tz=TimeZone(identifier:"Asia/Shanghai")!
var g=Calendar(identifier:.gregorian);g.timeZone=tz
var l=Calendar(identifier:.chinese);l.timeZone=tz
var starts:[(Int,Date)]=[]
for year in 2024...2101 {
 var d=g.date(from:DateComponents(year:year,month:1,day:1,hour:12))!
 for _ in 0..<65 {
  let c=l.dateComponents([.month,.day],from:d)
  if c.month==1 && c.day==1 && c.isLeapMonth != true {starts.append((year,d));break}
  d=g.date(byAdding:.day,value:1,to:d)!
 }
}
var rows:[[String:Int]]=[]
for i in 0..<(starts.count-1) {
 let (year,start)=starts[i];let end=starts[i+1].1
 var d=start;var leap=0;var mask=0;var lengths:[Int]=[];var previous=start
 while d<end {
  d=g.date(byAdding:.day,value:1,to:d)!
  if d==end || l.component(.day,from:d)==1 {
   lengths.append(g.dateComponents([.day],from:previous,to:d).day!);previous=d
   if d<end && l.dateComponents([.month],from:d).isLeapMonth==true {leap=l.component(.month,from:d)}
  }
 }
 for (i,len) in lengths.enumerated(){precondition(len==29||len==30);if len==30{mask |= 1<<i}}
 let c=g.dateComponents([.year,.month,.day],from:start)
 rows.append(["year":year,"new_year":c.year!*10000+c.month!*100+c.day!,"leap":leap,"mask":mask,"months":lengths.count])
}
print(String(data:try JSONSerialization.data(withJSONObject:rows,options:[.sortedKeys]),encoding:.utf8)!)
