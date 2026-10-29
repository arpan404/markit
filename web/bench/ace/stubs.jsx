// Stand-ins for Ace app components that need its client, router and stores.
// The Markdown pipeline, renderer, code blocks and math are Ace's own files.
import { createContext } from 'react';
export const ThreadReferenceScope = createContext(undefined);
export function ThreadReference(props) { return props.children; }
export function MarkdownImage(props) { return <span role="img" aria-label={props.alt} />; }
export function FileReferenceLink(props) { return props.code ? <code>{props.text}</code> : props.text; }
export function PullRequestLink({ threadId, pr, ...props }) { return <a {...props} />; }
export function pullRequestAt() { return undefined; }
export function WebLink(props) { return <a {...props} target="_blank" rel="noreferrer" />; }
export function SkeletonText(props) { return <div aria-busy="true" style={{ minHeight: `${(props.lines ?? 1) * 1.4}em` }} />; }
export function Skeleton() { return <div aria-busy="true" />; }
export function LoadingRegion(props) { return <div aria-busy="true">{props.children}</div>; }
export function ListSkeleton() { return <div aria-busy="true" />; }
