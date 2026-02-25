#!/usr/bin/env python3
import os, sys, argparse, subprocess, json

def run_koad_command(args):
    """Helper to run koad CLI commands."""
    try:
        subprocess.run(["koad"] + args, check=True, capture_output=True)
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False

def main():
    parser = argparse.ArgumentParser(description="KoadOS GCloud Ops (Integrated)")
    parser.add_argument('action', choices=['list', 'deploy', 'logs', 'audit'])
    parser.add_argument('--resource', default='functions')
    parser.add_argument('--name', help="Resource name for logs or deploy")
    parser.add_argument('--project', help="GCP Project ID")
    parser.add_argument('--limit', type=int, default=20)
    args = parser.parse_args()
    
    project_id = args.project or os.getenv("GCP_PROJECT")
    if not project_id:
        print("Error: GCP Project ID is required (via --project or GCP_PROJECT env var)")
        sys.exit(1)
    
    # If the native koad CLI passed the resource directly as a positional arg (old style)
    # We handle it here for backward compatibility with the GcloudAction enum in main.rs
    
    if args.action == 'list':
        if args.resource == 'functions':
            cmd = ['gcloud', 'functions', 'list', f'--project={project_id}']
        elif args.resource == 'run':
            cmd = ['gcloud', 'run', 'services', 'list', f'--project={project_id}']
        elif args.resource == 'iam':
            cmd = ['gcloud', 'projects', 'get-iam-policy', project_id, '--format=json']
        else:
            cmd = ['gcloud', args.resource, 'list', f'--project={project_id}']
        
        subprocess.run(cmd)

    elif args.action == 'audit':
        print(f"Auditing IAM for {project_id}...")
        cmd = ['gcloud', 'projects', 'get-iam-policy', project_id, '--format=json']
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode == 0:
            policy = json.loads(result.stdout)
            print(f"Found {len(policy.get('bindings', []))} IAM bindings.")
            run_koad_command(["stream", "post", f"IAM Audit: {project_id}", f"Found {len(policy.get('bindings', []))} IAM bindings during automated audit.", "--msg-type", "Log"])
        else:
            print(f"Audit failed: {result.stderr}")

    elif args.action == 'deploy':
        print(f"Dispatched deployment for {args.name} to {project_id}...")
        cmd = [
            'gcloud', 'functions', 'deploy', args.name,
            '--runtime=nodejs20', '--trigger-http', '--allow-unauthenticated',
            f'--project={project_id}', '--region=us-west1' # Default to us-west1 per findings
        ]
        subprocess.run(cmd)
        run_koad_command(["stream", "post", "Admin Deploy", f"User dispatched deployment for {args.name} to {project_id}.", "--msg-type", "Alert"])

    elif args.action == 'logs':
        cmd = ['gcloud', 'functions', 'logs', 'read', args.name, f'--limit={args.limit}', f'--project={project_id}']
        subprocess.run(cmd)

if __name__ == '__main__':
    main()
