const fs = require('fs');

function replaceInput(file, type) {
  let content = fs.readFileSync(file, 'utf-8');
  if (!content.includes('AutocompleteInput')) {
    content = "import AutocompleteInput from './AutocompleteInput';\n" + content;
  }
  
  if (type === 'livestatus') {
    content = content.replace(
      /<input[\s\S]*?pattern="\\d\{5\}"\s*\/>/,
      `<AutocompleteInput type="train" value={train} onChange={setTrain} placeholder="5-digit Train No." />`
    );
  } else if (type === 'livestation') {
    content = content.replace(
      /<input\s*type="text"\s*value=\{station\}\s*onChange=\{\(e\) => setStation\(e\.target\.value\.toUpperCase\(\)\)\}\s*placeholder="Station Code"\s*className="[^"]*"\s*required\s*\/>/,
      `<AutocompleteInput type="station" value={station} onChange={setStation} placeholder="Station Code" className="w-1/2" />`
    );
    // Might have slightly different class. Let's just use regex safely.
    content = content.replace(/<input[^>]*value=\{station\}[^>]*\/>/, `<AutocompleteInput type="station" value={station} onChange={setStation} placeholder="Station Code" className="w-1/2" />`);
  } else if (type === 'trainsbetween') {
    content = content.replace(/<input[^>]*value=\{src\}[^>]*\/>/, `<AutocompleteInput type="station" value={src} onChange={setSrc} placeholder="FROM" className="w-2/5" />`);
    content = content.replace(/<input[^>]*value=\{dst\}[^>]*\/>/, `<AutocompleteInput type="station" value={dst} onChange={setDst} placeholder="TO" className="w-2/5" />`);
  } else if (type === 'schedule') {
    content = content.replace(/<input[^>]*value=\{train\}[^>]*\/>/, `<AutocompleteInput type="train" value={train} onChange={setTrain} placeholder="5-digit Train No." />`);
  }

  fs.writeFileSync(file, content);
}

replaceInput('src/components/LiveStatusTab.tsx', 'livestatus');
replaceInput('src/components/LiveStationTab.tsx', 'livestation');
replaceInput('src/components/TrainsBetweenTab.tsx', 'trainsbetween');
replaceInput('src/components/ScheduleTab.tsx', 'schedule');
