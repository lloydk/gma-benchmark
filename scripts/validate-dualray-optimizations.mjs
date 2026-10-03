// Independent JS-oracle checks for isolated Dualray candidate source trees.
// Usage: node scripts/validate-dualray-optimizations.mjs --tree name=/path
//   --output /tmp/validation.json
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { RGB_SPACES } from '../src/rgb-spaces.js';
import { validateDualrayMethod } from '../tests/helpers/validate-dualray-method.js';
import { validateDualrayFastMethod } from '../tests/helpers/validate-dualray-fast-method.js';
import { dualraySamples } from '../tests/helpers/dualray-samples.js';
import { dualrayExperimentWorkloads } from './dualray-experiment-workloads.mjs';

const {values}=parseArgs({options:{tree:{type:'string',multiple:true},output:{type:'string'}}});
assert.ok(values.tree?.length && values.output);
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
function sources(tree,dir='src') {
 const hashes={};
 for(const name of readdirSync(resolve(tree,dir)).sort()) {
  const relative=`${dir}/${name}`,path=resolve(tree,relative);
  if(statSync(path).isDirectory())Object.assign(hashes,sources(tree,relative));
  else hashes[relative]=hash(readFileSync(path));
 }
 return hashes;
}
const origin=fileURLToPath(new URL('../',import.meta.url));
const report={schema:'dualray-optimization-validation-v1',runtime:process.versions,
 validatorSha256:hash(readFileSync(fileURLToPath(import.meta.url))),
 referenceHashes:{...sources(origin,'tests/helpers'),
  'scripts/dualray-experiment-workloads.mjs':hash(readFileSync(new URL('./dualray-experiment-workloads.mjs',import.meta.url)))},
 scope:'Separate validation seed plus existing adversarial boundary/fold/endpoint probes. XYZ first-exit oracle for mapped output; exact canonical conversion for Fast in-gamut output. Existing budgets unchanged.',
 trees:[],results:[]};
for(const value of values.tree) {
 const at=value.indexOf('=');assert.ok(at>0);
 const name=value.slice(0,at),path=resolve(value.slice(at+1));
 report.trees.push({name,path,sourceHashes:sources(path)});
 const {RGB_SPACES:spaces}=await import(pathToFileURL(resolve(path,'src/rgb-spaces.js')));
 const {createDualray}=await import(pathToFileURL(resolve(path,'src/dualray-factory.js')));
 const {createDualrayFast}=await import(pathToFileURL(resolve(path,'src/dualray-fast.js')));
 for(const space of Object.values(spaces)) {
  const sets=[...Object.values(dualrayExperimentWorkloads(space.id,true)),dualraySamples(space.id)];
  for(const [method,validate,map] of [
   ['dualray',validateDualrayMethod,createDualray(space)],
   ['dualray-fast',validateDualrayFastMethod,createDualrayFast(space)],
  ]) {
   try {
    const result=validate(RGB_SPACES[space.id],sets,map);
    report.results.push({tree:name,method,...result});
    console.log(`${name}/${space.id}/${method}: ${result.count} passed`);
   } catch(error) {report.results.push({tree:name,gamut:space.id,method,failure:error.message});console.error(error.message);}
  }
 }
}
report.completed=new Date().toISOString();
writeFileSync(values.output,JSON.stringify(report,null,2)+'\n');
if(report.results.some(row=>row.failure))process.exitCode=1;
