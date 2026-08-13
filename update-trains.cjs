const fs = require('fs');

try {
  let raw = fs.readFileSync('data/trains.json', 'utf-8');
  let trains = JSON.parse(raw);
  
  const moreTrains = [
    { number: "17602", name: "SGNR KCG EXP" },
    { number: "17601", name: "KCG SGNR EXP" }
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
