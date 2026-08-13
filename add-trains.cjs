const fs = require('fs');

try {
  let raw = fs.readFileSync('data/trains.json', 'utf-8');
  let trains = JSON.parse(raw);
  
  // Fix 12129 and 12130
  trains = trains.map(t => {
    if (t.number === "12129") return { number: "12129", name: "AZAD HIND EXP" };
    if (t.number === "12130") return { number: "12130", name: "AZAD HIND EXP" };
    return t;
  });
  
  // Add some more just in case
  const moreTrains = [
    { number: "12833", name: "HOWRAH EXP" },
    { number: "12834", name: "HOWRAH EXP" },
    { number: "11077", name: "JHELUM EXP" },
    { number: "11078", name: "JHELUM EXP" },
    { number: "12139", name: "SEVAGRAM EXP" },
    { number: "12140", name: "SEVAGRAM EXP" },
    { number: "12779", name: "GOA EXPRESS" },
    { number: "12780", name: "GOA EXPRESS" },
    { number: "12295", name: "SANGHAMITRA EXP" },
    { number: "12296", name: "SANGHAMITRA EXP" }
  ];
  
  moreTrains.forEach(t => {
    if (!trains.find(existing => existing.number === t.number)) {
      trains.push(t);
    }
  });

  fs.writeFileSync('data/trains.json', JSON.stringify(trains, null, 2));
} catch(e) {
  console.error("Error updating trains:", e);
}
