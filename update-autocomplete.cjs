const fs = require('fs');

let code = fs.readFileSync('src/components/AutocompleteInput.tsx', 'utf-8');

code = code.replace(
  /onChange=\{\(e\) => \{\s*setQuery\(e\.target\.value\.toUpperCase\(\)\);\s*if \(e\.target\.value === ''\) onChange\(''\);\s*setIsOpen\(true\);\s*\}\}/,
  `onChange={(e) => {
          const val = e.target.value.toUpperCase();
          setQuery(val);
          onChange(val);
          setIsOpen(true);
        }}`
);

fs.writeFileSync('src/components/AutocompleteInput.tsx', code);
