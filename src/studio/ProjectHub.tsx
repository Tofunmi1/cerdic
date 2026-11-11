import { useEffect, useRef, useState } from 'react';
import { ArrowRight, FileUp, Folder, FolderPlus, Pencil, Plus, Search, X } from 'lucide-react';
import { EXAMPLES } from './examples';
import type { Project } from './library/projects';
import { workspace } from './library/workspace';

type Props = { projects: Project[]; active: Project | null; busy: boolean; onClose: () => void };
export function ProjectHub({ projects, active, busy, onClose }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const [name, setName] = useState('');
  const [source, setSource] = useState('blank');
  const [file, setFile] = useState<File | null>(null);
  const [query, setQuery] = useState('');
  const [working, setWorking] = useState(false);
  const [error, setError] = useState('');
  const [editing, setEditing] = useState<string | null>(null);
  const [rename, setRename] = useState('');
  const disabled = busy || working;
  useEffect(() => { const el = dialog.current; el?.showModal(); return () => el?.close(); }, []);
  const run = async (work: () => Promise<unknown>, close = true) => {
    setError(''); setWorking(true);
    try { await work(); if (close) onClose(); }
    catch (problem) { setError(problem instanceof Error ? problem.message : String(problem)); }
    finally { setWorking(false); }
  };
  const create = () => run(async () => {
    let text = '', origin = source;
    if (source === 'file') {
      if (!file) throw new Error('Choose a scene file first.');
      text = await file.text(); origin = 'file';
    } else if (source !== 'blank') text = EXAMPLES.find(e => e.id === source)?.text ?? '';
    await workspace.create(name.trim() || 'Untitled project', text, origin);
  });
  return (
    <dialog className="project-hub glass strong" ref={dialog} aria-labelledby="projects-title" onCancel={event => { event.preventDefault(); if (!disabled && active) onClose(); }}>
      <header className="project-hub-head"><div><span className="project-eyebrow">YOUR WORKSPACE</span><h1 id="projects-title">A home for every world.</h1><p>Projects keep your scene and its conversations together in this browser.</p></div>{active && <button className="btn icon" onClick={onClose} disabled={disabled} aria-label="Close projects"><X size={18} /></button>}</header>
      <div className="project-hub-body">
        <form className="project-create" onSubmit={event => { event.preventDefault(); void create(); }}>
          <span className="project-create-icon"><FolderPlus size={24} /></span><h2>Create a project</h2><p>Start with one world. Its chats and edits stay with it.</p>
          <label>Project name<input autoFocus value={name} onChange={event => setName(event.target.value)} placeholder="My next world" maxLength={100} disabled={disabled} /></label>
          <label>Starting point<select aria-label="Starting point" value={source} disabled={disabled} onChange={event => setSource(event.target.value)}><option value="blank">Blank world</option><option value="file">Import a scene file</option><optgroup label="Example worlds">{EXAMPLES.map(example => <option value={example.id} key={example.id}>{example.title}</option>)}</optgroup></select></label>
          {source === 'file' && <><input ref={fileInput} type="file" accept=".txt,.world,.ve,text/plain" hidden onChange={event => { const selected = event.target.files?.[0]; if (selected) { setFile(selected); if (!name.trim()) setName(selected.name.replace(/\.[^.]+$/, '')); } }} /><button className="btn project-file" type="button" disabled={disabled} onClick={() => fileInput.current?.click()}><FileUp size={15} />{file?.name ?? 'Choose scene file'}</button></>}
          <button className="btn primary project-create-submit" disabled={disabled} type="submit"><Plus size={16} />{disabled ? 'Opening world…' : 'Create project'}<ArrowRight size={16} /></button>
          <span className="project-local-note">Saved locally · export your scene to keep a separate copy</span>
        </form>
        <section className="project-recents" aria-label="Saved projects"><div className="project-recents-head"><h2>Your projects <span>{projects.length}</span></h2><label className="project-search"><Search size={15} /><input value={query} onChange={event => setQuery(event.target.value)} placeholder="Find a project" aria-label="Find a project" /></label></div>
          <div className="project-list">{projects.filter(p => p.name.toLowerCase().includes(query.toLowerCase())).map(project => <article className="project-row" key={project.id} data-current={active?.id === project.id}>
            <button className="project-open" disabled={disabled} onClick={() => void run(() => workspace.open(project))}><span className="project-folder"><Folder size={22} /></span><span><strong>{project.name}</strong><small>1 world · {new Date(project.updatedAt).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}{active?.id === project.id ? ' · Current project' : ''}</small></span><ArrowRight size={15} /></button>
            <button className="btn icon" disabled={disabled} aria-label={`Rename ${project.name}`} onClick={() => { setEditing(project.id); setRename(project.name); }}><Pencil size={14} /></button>
            {editing === project.id && <form className="project-rename" onSubmit={event => { event.preventDefault(); if (rename.trim()) void run(async () => { await workspace.rename(project, rename); setEditing(null); }, false); }}><input aria-label="New project name" value={rename} onChange={event => setRename(event.target.value)} maxLength={100} /><button className="btn" disabled={disabled || !rename.trim()}>Save name</button><button className="btn" type="button" onClick={() => setEditing(null)}>Cancel</button></form>}
          </article>)}{projects.length === 0 && <div className="project-list-empty"><Folder size={32} /><h3>Your first world starts here.</h3><p>Create a blank project, bring a scene file, or make an example your own.</p></div>}{projects.length > 0 && !projects.some(p => p.name.toLowerCase().includes(query.toLowerCase())) && <p className="project-list-empty">No projects match your search.</p>}</div>
        </section>
      </div>
      {error && <p className="project-error" role="alert">{error}</p>}
    </dialog>
  );
}
