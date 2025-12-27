# Admin Interface Guide

## Overview

A comprehensive admin interface has been added to the Network Monitor system, providing fast and efficient management of users, roles, and regions with a context-sensitive menu and keyboard shortcuts.

## Features

### ✅ User Management
- Create, edit, and delete users
- Assign roles to users
- Assign regions to users
- Activate/deactivate users
- Search and filter users
- Real-time statistics (total users, active users)

### ✅ Role Management
- Create, edit, and delete roles
- Define permissions (read, write, admin, delete)
- Search and filter roles
- Real-time statistics

### ✅ Region Management
- Create, edit, and delete regions
- Set region codes (e.g., "us-east", "eu-west")
- Activate/deactivate regions
- Search and filter regions
- Real-time statistics (total regions, active regions)

### ✅ Context-Sensitive Menu
- Sidebar navigation with active state indicators
- Quick access to all management sections
- Visual feedback for current section

### ✅ Keyboard Shortcuts
- **Ctrl+U** (Cmd+U on Mac): Switch to Users section
- **Ctrl+R** (Cmd+R on Mac): Switch to Roles section
- **Ctrl+G** (Cmd+G on Mac): Switch to Regions section
- **Escape**: Close any open modal

## Access

1. Start the monitor:
   ```bash
   cargo run --release --bin monitor
   ```

2. Open the admin panel:
   - Navigate to: `http://localhost:8080/admin.html`
   - Or click the "⚙️ Admin Panel" button on the main dashboard

## API Endpoints

### Users
- `GET /api/admin/users` - List all users
- `POST /api/admin/users` - Create a new user
- `GET /api/admin/users/:id` - Get a specific user
- `PUT /api/admin/users/:id` - Update a user
- `DELETE /api/admin/users/:id` - Delete a user

### Roles
- `GET /api/admin/roles` - List all roles
- `POST /api/admin/roles` - Create a new role
- `GET /api/admin/roles/:id` - Get a specific role
- `PUT /api/admin/roles/:id` - Update a role
- `DELETE /api/admin/roles/:id` - Delete a role

### Regions
- `GET /api/admin/regions` - List all regions
- `POST /api/admin/regions` - Create a new region
- `GET /api/admin/regions/:id` - Get a specific region
- `PUT /api/admin/regions/:id` - Update a region
- `DELETE /api/admin/regions/:id` - Delete a region

## Default Data

The system initializes with:

### Default Roles
- **Administrator**: Full access (read, write, admin, delete)
- **User**: Read-only access

### Default Regions
- **US East** (us-east): United States East Coast
- **EU West** (eu-west): Europe West

## Usage Examples

### Creating a User

1. Click "Users" in the sidebar (or press Ctrl+U)
2. Click "+ Add User"
3. Fill in:
   - Username
   - Email
   - Select roles (checkboxes)
   - Select regions (checkboxes)
   - Active status
4. Click "Save"

### Creating a Role

1. Click "Roles" in the sidebar (or press Ctrl+R)
2. Click "+ Add Role"
3. Fill in:
   - Name
   - Description
   - Permissions (checkboxes: read, write, admin, delete)
4. Click "Save"

### Creating a Region

1. Click "Regions" in the sidebar (or press Ctrl+G)
2. Click "+ Add Region"
3. Fill in:
   - Name
   - Code (unique identifier, e.g., "us-west")
   - Description
   - Active status
4. Click "Save"

## UI Features

### Search Functionality
- Each section has a search box
- Real-time filtering as you type
- Searches across all visible fields

### Statistics Cards
- Colorful gradient cards showing key metrics
- Updates automatically when data changes

### Responsive Design
- Works on desktop and tablet screens
- Sticky sidebar navigation
- Modal dialogs for forms

### Visual Feedback
- Active navigation items highlighted
- Status badges (Active/Inactive)
- Permission badges for roles
- Hover effects on interactive elements

## Data Persistence

**Note**: Currently, all data is stored in memory. When the monitor restarts, all admin data (except defaults) will be reset. For production use, consider adding database persistence.

## Future Enhancements

Potential improvements:
- [ ] Database persistence
- [ ] User authentication
- [ ] Role-based access control for the admin panel
- [ ] Bulk operations (bulk delete, bulk assign)
- [ ] Export/import functionality
- [ ] Audit logging
- [ ] Advanced filtering and sorting
- [ ] User activity tracking

## Technical Details

### Backend
- Data structures: `User`, `Role`, `Region`
- Admin state stored in `AdminState` struct
- RESTful API using Axum
- JSON request/response format

### Frontend
- Pure HTML/CSS/JavaScript (no frameworks)
- Modern gradient design
- Responsive layout
- Real-time updates via API calls

## Troubleshooting

### Admin panel not loading
- Ensure `web/admin.html` exists
- Check browser console for errors
- Verify monitor is running on correct port

### API errors
- Check monitor logs for backend errors
- Verify API endpoints are accessible
- Check browser network tab for request/response details

### Data not persisting
- This is expected behavior - data is in-memory only
- Consider implementing database storage for production

## Support

For issues or questions:
1. Check monitor logs
2. Review browser console
3. Verify API endpoints are working
4. Check network requests in browser dev tools












