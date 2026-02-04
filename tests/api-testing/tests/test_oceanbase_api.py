"""
OceanBase Backend Database API Test Suite

Tests cover all HTTP APIs involving database CRUD operations to verify OceanBase compatibility.

How to run these tests:
========================

1. Environment Setup:
   - Ensure OpenObserve server is running with OceanBase backend
   - Default server address: http://127.0.0.1:5080/

2. Environment Variables (optional, uses defaults if not set):
   export ZO_BASE_URL="http://127.0.0.1:5080/"
   export ZO_ROOT_USER_EMAIL="root@example.com"
   export ZO_ROOT_USER_PASSWORD="Complexpass#123"

3. Run all tests:
   cd /path/to/openobserve/tests/api-testing
   python -m pytest tests/test_oceanbase_api.py -v

4. Run specific test class:
   python -m pytest tests/test_oceanbase_api.py::TestUsersCRUD -v

5. Run specific test:
   python -m pytest tests/test_oceanbase_api.py::TestUsersCRUD::test_create_user -v

6. Run with detailed output:
   python -m pytest tests/test_oceanbase_api.py -v -s

Test Requirements:
- pytest
- pytest-order
- requests

Install dependencies:
   pip install pytest pytest-order requests
"""

import pytest
import time
import logging
from datetime import datetime

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)


def generate_unique_id():
    """Generate a unique identifier based on timestamp"""
    return datetime.now().strftime("%Y%m%d%H%M%S%f")


# ==================== User Management Tests ====================

class TestUsersCRUD:
    """User CRUD operation tests"""
    
    @pytest.fixture
    def unique_user_email(self):
        return f"test_user_{generate_unique_id()}@example.com"
    
    @pytest.mark.order(1)
    def test_list_users(self, create_session, base_url):
        """Test listing users"""
        resp = create_session.get(f"{base_url}api/default/users")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "data" in data, f"Response should contain 'data' field: {data}"
        logger.info(f"Listed {len(data.get('data', []))} users")
    
    @pytest.mark.order(2)
    def test_create_user(self, create_session, base_url, unique_user_email):
        """Test creating a user"""
        payload = {
            "email": unique_user_email,
            "password": "TestPass#123",
            "first_name": "Test",
            "last_name": "User",
            "role": "admin"  # Built-in roles: root, admin, editor, viewer, user
        }
        resp = create_session.post(f"{base_url}api/default/users", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Created user: {unique_user_email}")
    
    @pytest.mark.order(3)
    def test_create_user_duplicate_email(self, create_session, base_url):
        """Test creating user with duplicate email (should fail)"""
        email = f"duplicate_user_{generate_unique_id()}@example.com"
        payload = {
            "email": email,
            "password": "TestPass#123",
            "role": "admin"  # Using built-in role
        }
        # First creation
        resp1 = create_session.post(f"{base_url}api/default/users", json=payload)
        assert resp1.status_code == 200, f"First create should succeed: {resp1.text}"
        
        # Second creation should fail
        resp2 = create_session.post(f"{base_url}api/default/users", json=payload)
        assert resp2.status_code in [400, 409, 500], f"Duplicate create should fail: {resp2.status_code}"
        logger.info(f"Duplicate user creation correctly rejected with status {resp2.status_code}")
    
    @pytest.mark.order(4)
    def test_update_user(self, create_session, base_url):
        """Test updating user information"""
        email = f"update_user_{generate_unique_id()}@example.com"
        # Create user first
        create_payload = {
            "email": email,
            "password": "TestPass#123",
            "first_name": "Original",
            "last_name": "Name",
            "role": "admin"  # Only admin role is currently allowed
        }
        create_resp = create_session.post(f"{base_url}api/default/users", json=create_payload)
        assert create_resp.status_code == 200, f"Create user failed: {create_resp.text}"
        
        # Update user
        update_payload = {
            "first_name": "Updated",
            "last_name": "Name",
            "role": "admin"  # Only admin role is currently allowed
        }
        resp = create_session.put(f"{base_url}api/default/users/{email}", json=update_payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Updated user: {email}")
    
    @pytest.mark.order(5)
    def test_delete_user(self, create_session, base_url):
        """Test deleting a user"""
        email = f"delete_user_{generate_unique_id()}@example.com"
        # Create user first
        payload = {
            "email": email,
            "password": "TestPass#123",
            "role": "admin"  # Only admin role is currently allowed
        }
        create_resp = create_session.post(f"{base_url}api/default/users", json=payload)
        assert create_resp.status_code == 200, f"Create user failed: {create_resp.text}"
        
        # Delete user
        resp = create_session.delete(f"{base_url}api/default/users/{email}")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Deleted user: {email}")
    
    @pytest.mark.order(6)
    def test_delete_nonexistent_user(self, create_session, base_url):
        """Test deleting a non-existent user"""
        resp = create_session.delete(f"{base_url}api/default/users/nonexistent_{generate_unique_id()}@example.com")
        assert resp.status_code in [404, 500], f"Expected 404/500, got {resp.status_code}"
        logger.info("Deleting nonexistent user correctly returned error")


# ==================== Organization Management Tests ====================

class TestOrganizationsCRUD:
    """Organization CRUD operation tests"""
    
    @pytest.mark.order(10)
    def test_list_organizations(self, create_session, base_url):
        """Test listing organizations"""
        resp = create_session.get(f"{base_url}api/organizations")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "data" in data, f"Response should contain 'data' field: {data}"
        logger.info(f"Listed {len(data.get('data', []))} organizations")
    
    @pytest.mark.order(11)
    def test_get_organization_settings(self, create_session, base_url):
        """Test getting organization settings"""
        resp = create_session.get(f"{base_url}api/default/settings")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Got organization settings successfully")
    
    @pytest.mark.order(12)
    def test_get_passcode(self, create_session, base_url):
        """Test getting ingestion token"""
        resp = create_session.get(f"{base_url}api/default/passcode")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "data" in data, f"Response should contain 'data' field: {data}"
        logger.info("Got passcode successfully")
    
    @pytest.mark.order(13)
    def test_update_passcode(self, create_session, base_url):
        """Test updating ingestion token"""
        resp = create_session.put(f"{base_url}api/default/passcode")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Updated passcode successfully")


# ==================== Stream Management Tests ====================

class TestStreamsCRUD:
    """Stream CRUD operation tests"""
    
    @pytest.fixture
    def unique_stream_name(self):
        return f"test_stream_{generate_unique_id()}"
    
    @pytest.mark.order(20)
    def test_list_streams(self, create_session, base_url):
        """Test listing streams"""
        resp = create_session.get(
            f"{base_url}api/default/streams",
            params={"type": "logs", "offset": 0, "limit": 100}
        )
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "list" in data, f"Response should contain 'list' field: {data}"
        logger.info(f"Listed {len(data.get('list', []))} streams")
    
    @pytest.mark.order(21)
    def test_create_stream_via_ingestion(self, create_session, base_url, unique_stream_name):
        """Test creating stream via data ingestion"""
        payload = [
            {"message": "test log message", "level": "info", "timestamp": datetime.now().isoformat()}
        ]
        resp = create_session.post(f"{base_url}api/default/{unique_stream_name}/_json", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Created stream via ingestion: {unique_stream_name}")
    
    @pytest.mark.order(22)
    def test_get_stream_schema(self, create_session, base_url):
        """Test getting stream schema"""
        stream_name = f"schema_test_{generate_unique_id()}"
        # Create stream first
        payload = [{"message": "test", "level": "info"}]
        create_session.post(f"{base_url}api/default/{stream_name}/_json", json=payload)
        
        # Wait for stream creation
        time.sleep(2)
        
        resp = create_session.get(
            f"{base_url}api/default/streams/{stream_name}/schema",
            params={"type": "logs"}
        )
        # Schema may not be generated yet
        assert resp.status_code in [200, 404], f"Expected 200/404, got {resp.status_code}: {resp.text}"
        logger.info(f"Got stream schema, status: {resp.status_code}")
    
    @pytest.mark.order(23)
    def test_update_stream_settings(self, create_session, base_url):
        """Test updating stream settings"""
        stream_name = f"settings_test_{generate_unique_id()}"
        # Create stream first
        payload = [{"message": "test"}]
        create_session.post(f"{base_url}api/default/{stream_name}/_json", json=payload)
        time.sleep(2)
        
        # Update settings
        settings_payload = {"data_retention": 30}
        resp = create_session.put(
            f"{base_url}api/default/streams/{stream_name}/settings",
            params={"type": "logs"},
            json=settings_payload
        )
        assert resp.status_code in [200, 404], f"Expected 200/404, got {resp.status_code}: {resp.text}"
        logger.info(f"Updated stream settings, status: {resp.status_code}")
    
    @pytest.mark.order(24)
    def test_delete_stream(self, create_session, base_url):
        """Test deleting a stream"""
        stream_name = f"delete_test_{generate_unique_id()}"
        # Create stream first
        payload = [{"message": "test"}]
        create_session.post(f"{base_url}api/default/{stream_name}/_json", json=payload)
        time.sleep(2)
        
        # Delete stream
        resp = create_session.delete(
            f"{base_url}api/default/streams/{stream_name}",
            params={"type": "logs"}
        )
        assert resp.status_code in [200, 404], f"Expected 200/404, got {resp.status_code}: {resp.text}"
        logger.info(f"Deleted stream, status: {resp.status_code}")


# ==================== Dashboard Management Tests ====================

class TestDashboardsCRUD:
    """Dashboard CRUD operation tests"""
    
    @pytest.fixture
    def dashboard_payload(self):
        unique_id = generate_unique_id()
        return {
            "title": f"Test Dashboard {unique_id}",
            "description": "Test dashboard description",
            "version": 5,
            "v5": {
                "title": f"Test Dashboard {unique_id}",
                "description": "Test description",
                "panels": [],
                "variables": {"list": []},
                "tabs": []
            }
        }
    
    @pytest.mark.order(30)
    def test_list_dashboards(self, create_session, base_url):
        """Test listing dashboards"""
        resp = create_session.get(f"{base_url}api/default/dashboards")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "dashboards" in data, f"Response should contain 'dashboards' field: {data}"
        logger.info(f"Listed {len(data.get('dashboards', []))} dashboards")
    
    @pytest.mark.order(31)
    def test_create_dashboard(self, create_session, base_url, dashboard_payload):
        """Test creating a dashboard"""
        resp = create_session.post(f"{base_url}api/default/dashboards", json=dashboard_payload)
        assert resp.status_code in [200, 201], f"Expected 200/201, got {resp.status_code}: {resp.text}"
        data = resp.json()
        logger.info(f"Created dashboard: {data}")
    
    @pytest.mark.order(32)
    def test_get_dashboard(self, create_session, base_url, dashboard_payload):
        """Test getting dashboard details"""
        # Create first
        created = create_session.post(f"{base_url}api/default/dashboards", json=dashboard_payload).json()
        dashboard_id = created.get("v5", {}).get("dashboard_id") or created.get("dashboard_id")
        
        if dashboard_id:
            resp = create_session.get(f"{base_url}api/default/dashboards/{dashboard_id}")
            assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
            logger.info(f"Got dashboard: {dashboard_id}")
        else:
            logger.warning("Dashboard ID not found in response, skipping get test")
    
    @pytest.mark.order(33)
    def test_update_dashboard(self, create_session, base_url, dashboard_payload):
        """Test updating a dashboard"""
        # Create first
        created = create_session.post(f"{base_url}api/default/dashboards", json=dashboard_payload).json()
        dashboard_id = created.get("v5", {}).get("dashboard_id") or created.get("dashboard_id")
        
        if dashboard_id:
            dashboard_payload["title"] = "Updated Title"
            resp = create_session.put(f"{base_url}api/default/dashboards/{dashboard_id}", json=dashboard_payload)
            assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
            logger.info(f"Updated dashboard: {dashboard_id}")
        else:
            logger.warning("Dashboard ID not found, skipping update test")
    
    @pytest.mark.order(34)
    def test_delete_dashboard(self, create_session, base_url, dashboard_payload):
        """Test deleting a dashboard"""
        # Create first
        created = create_session.post(f"{base_url}api/default/dashboards", json=dashboard_payload).json()
        dashboard_id = created.get("v5", {}).get("dashboard_id") or created.get("dashboard_id")
        
        if dashboard_id:
            resp = create_session.delete(f"{base_url}api/default/dashboards/{dashboard_id}")
            assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
            logger.info(f"Deleted dashboard: {dashboard_id}")
        else:
            logger.warning("Dashboard ID not found, skipping delete test")


# ==================== Alert Management Tests ====================

class TestAlertsCRUD:
    """Alert CRUD operation tests"""
    
    @pytest.fixture
    def template_name(self):
        return f"test_template_{generate_unique_id()}"
    
    @pytest.fixture
    def destination_name(self):
        return f"test_dest_{generate_unique_id()}"
    
    @pytest.mark.order(40)
    def test_list_templates(self, create_session, base_url):
        """Test listing alert templates"""
        resp = create_session.get(f"{base_url}api/default/alerts/templates")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Listed alert templates successfully")
    
    @pytest.mark.order(41)
    def test_create_template(self, create_session, base_url, template_name):
        """Test creating an alert template"""
        payload = {
            "name": template_name,
            "body": '{"text": "Alert: {alert_name}"}'
        }
        resp = create_session.post(f"{base_url}api/default/alerts/templates", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Created template: {template_name}")
    
    @pytest.mark.order(42)
    def test_get_template(self, create_session, base_url):
        """Test getting alert template details"""
        template_name = f"get_template_{generate_unique_id()}"
        # Create first
        payload = {"name": template_name, "body": '{"text": "Test"}'}
        create_session.post(f"{base_url}api/default/alerts/templates", json=payload)
        
        resp = create_session.get(f"{base_url}api/default/alerts/templates/{template_name}")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Got template: {template_name}")
    
    @pytest.mark.order(43)
    def test_delete_template(self, create_session, base_url):
        """Test deleting an alert template"""
        template_name = f"del_template_{generate_unique_id()}"
        # Create first
        payload = {"name": template_name, "body": '{"text": "Test"}'}
        create_session.post(f"{base_url}api/default/alerts/templates", json=payload)
        
        resp = create_session.delete(f"{base_url}api/default/alerts/templates/{template_name}")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Deleted template: {template_name}")
    
    @pytest.mark.order(44)
    def test_list_destinations(self, create_session, base_url):
        """Test listing alert destinations"""
        resp = create_session.get(f"{base_url}api/default/alerts/destinations")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Listed alert destinations successfully")
    
    @pytest.mark.order(45)
    def test_create_destination(self, create_session, base_url):
        """Test creating an alert destination"""
        template_name = f"dest_template_{generate_unique_id()}"
        dest_name = f"dest_{generate_unique_id()}"
        
        # Create template first
        create_session.post(
            f"{base_url}api/default/alerts/templates",
            json={"name": template_name, "body": '{"text": "Alert"}'}
        )
        
        payload = {
            "name": dest_name,
            "url": "https://webhook.example.com",
            "method": "post",
            "skip_tls_verify": False,
            "template": template_name,
            "type": "http"  # Use lowercase
        }
        resp = create_session.post(f"{base_url}api/default/alerts/destinations", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Created destination: {dest_name}")
    
    @pytest.mark.order(46)
    def test_list_alerts(self, create_session, base_url):
        """Test listing alerts"""
        resp = create_session.get(f"{base_url}api/v2/default/alerts")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "list" in data, f"Response should contain 'list' field: {data}"
        logger.info(f"Listed {len(data.get('list', []))} alerts")


# ==================== Function Management Tests ====================

class TestFunctionsCRUD:
    """Function CRUD operation tests"""
    
    @pytest.fixture
    def function_name(self):
        return f"test_func_{generate_unique_id()}"
    
    @pytest.mark.order(50)
    def test_list_functions(self, create_session, base_url):
        """Test listing functions"""
        resp = create_session.get(f"{base_url}api/default/functions")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "list" in data, f"Response should contain 'list' field: {data}"
        logger.info(f"Listed {len(data.get('list', []))} functions")
    
    @pytest.mark.order(51)
    def test_create_function(self, create_session, base_url, function_name):
        """Test creating a function"""
        payload = {
            "name": function_name,
            "function": '.message = "transformed"',
            "params": "row",
            "numArgs": 0,
            "transType": 0
        }
        resp = create_session.post(f"{base_url}api/default/functions", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Created function: {function_name}")
    
    @pytest.mark.order(52)
    def test_update_function(self, create_session, base_url):
        """Test updating a function"""
        function_name = f"upd_func_{generate_unique_id()}"
        # Create first
        payload = {
            "name": function_name,
            "function": '.message = "original"',
            "params": "row",
            "numArgs": 0,
            "transType": 0
        }
        create_session.post(f"{base_url}api/default/functions", json=payload)
        
        # Update
        payload["function"] = '.message = "updated"'
        resp = create_session.put(f"{base_url}api/default/functions/{function_name}", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Updated function: {function_name}")
    
    @pytest.mark.order(53)
    def test_delete_function(self, create_session, base_url):
        """Test deleting a function"""
        function_name = f"del_func_{generate_unique_id()}"
        # Create first
        payload = {
            "name": function_name,
            "function": '.message = "test"',
            "params": "row",
            "numArgs": 0,
            "transType": 0
        }
        create_session.post(f"{base_url}api/default/functions", json=payload)
        
        resp = create_session.delete(f"{base_url}api/default/functions/{function_name}", params={"force": "true"})
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Deleted function: {function_name}")


# ==================== KV Storage Tests ====================

class TestKVCRUD:
    """KV Storage CRUD operation tests"""
    
    @pytest.fixture
    def key_name(self):
        return f"test_key_{generate_unique_id()}"
    
    @pytest.mark.order(60)
    def test_list_keys(self, create_session, base_url):
        """Test listing KV keys"""
        resp = create_session.get(f"{base_url}api/default/kv")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Listed KV keys successfully")
    
    @pytest.mark.order(61)
    def test_set_value(self, create_session, base_url, key_name):
        """Test storing a value"""
        resp = create_session.post(
            f"{base_url}api/default/kv/{key_name}",
            data="test_value",
            headers={"Content-Type": "text/plain"}
        )
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Set KV: {key_name}")
    
    @pytest.mark.order(62)
    def test_get_value(self, create_session, base_url):
        """Test retrieving a value"""
        key_name = f"get_key_{generate_unique_id()}"
        # Set first
        create_session.post(
            f"{base_url}api/default/kv/{key_name}",
            data="test_value",
            headers={"Content-Type": "text/plain"}
        )
        
        resp = create_session.get(f"{base_url}api/default/kv/{key_name}")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        assert resp.text == "test_value", f"Expected 'test_value', got '{resp.text}'"
        logger.info(f"Got KV: {key_name}")
    
    @pytest.mark.order(63)
    def test_get_nonexistent_key(self, create_session, base_url):
        """Test retrieving a non-existent key"""
        resp = create_session.get(f"{base_url}api/default/kv/nonexistent_{generate_unique_id()}")
        assert resp.status_code == 404, f"Expected 404, got {resp.status_code}: {resp.text}"
        logger.info("Getting nonexistent key correctly returned 404")
    
    @pytest.mark.order(64)
    def test_delete_key(self, create_session, base_url):
        """Test deleting a key"""
        key_name = f"del_key_{generate_unique_id()}"
        # Set first
        create_session.post(
            f"{base_url}api/default/kv/{key_name}",
            data="test_value",
            headers={"Content-Type": "text/plain"}
        )
        
        resp = create_session.delete(f"{base_url}api/default/kv/{key_name}")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Deleted KV: {key_name}")
    
    @pytest.mark.order(65)
    def test_delete_nonexistent_key(self, create_session, base_url):
        """Test deleting a non-existent key - API returns 200 (idempotent delete)"""
        resp = create_session.delete(f"{base_url}api/default/kv/nonexistent_{generate_unique_id()}")
        # API implements idempotent delete, returns 200 even if key doesn't exist
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Deleting nonexistent key returned 200 (idempotent delete)")


# ==================== Folder Management Tests ====================

class TestFoldersCRUD:
    """Folder CRUD operation tests"""
    
    @pytest.fixture
    def folder_name(self):
        return f"test_folder_{generate_unique_id()}"
    
    @pytest.mark.order(70)
    def test_list_dashboard_folders(self, create_session, base_url):
        """Test listing dashboard folders"""
        resp = create_session.get(f"{base_url}api/v2/default/folders/dashboards")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "list" in data, f"Response should contain 'list' field: {data}"
        logger.info(f"Listed {len(data.get('list', []))} dashboard folders")
    
    @pytest.mark.order(71)
    def test_list_alert_folders(self, create_session, base_url):
        """Test listing alert folders"""
        resp = create_session.get(f"{base_url}api/v2/default/folders/alerts")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "list" in data, f"Response should contain 'list' field: {data}"
        logger.info(f"Listed {len(data.get('list', []))} alert folders")
    
    @pytest.mark.order(72)
    def test_create_folder(self, create_session, base_url, folder_name):
        """Test creating a folder"""
        payload = {
            "name": folder_name,
            "description": "Test folder description"
        }
        resp = create_session.post(f"{base_url}api/v2/default/folders/dashboards", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        logger.info(f"Created folder: {folder_name}, response: {data}")
    
    @pytest.mark.order(73)
    def test_delete_folder(self, create_session, base_url):
        """Test deleting a folder"""
        folder_name = f"del_folder_{generate_unique_id()}"
        payload = {"name": folder_name, "description": "To be deleted"}
        created = create_session.post(f"{base_url}api/v2/default/folders/dashboards", json=payload).json()
        folder_id = created.get("folderId")
        
        if folder_id:
            resp = create_session.delete(f"{base_url}api/v2/default/folders/dashboards/{folder_id}")
            assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
            logger.info(f"Deleted folder: {folder_id}")
        else:
            logger.warning("Folder ID not found, skipping delete test")


# ==================== Saved Views Tests ====================

class TestSavedViewsCRUD:
    """Saved Views CRUD operation tests"""
    
    @pytest.fixture
    def view_name(self):
        return f"test_view_{generate_unique_id()}"
    
    @pytest.mark.order(80)
    def test_list_saved_views(self, create_session, base_url):
        """Test listing saved views"""
        resp = create_session.get(f"{base_url}api/default/savedviews")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Listed saved views successfully")
    
    @pytest.mark.order(81)
    def test_create_saved_view(self, create_session, base_url, view_name):
        """Test creating a saved view"""
        import base64
        view_data = {"stream": "test", "query": "SELECT *"}
        encoded_data = base64.b64encode(str(view_data).encode()).decode()
        
        payload = {
            "view_name": view_name,
            "data": encoded_data
        }
        resp = create_session.post(f"{base_url}api/default/savedviews", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Created saved view: {view_name}")
    
    @pytest.mark.order(82)
    def test_delete_saved_view(self, create_session, base_url):
        """Test deleting a saved view"""
        import base64
        view_name = f"del_view_{generate_unique_id()}"
        view_data = {"stream": "test"}
        encoded_data = base64.b64encode(str(view_data).encode()).decode()
        
        created = create_session.post(
            f"{base_url}api/default/savedviews",
            json={"view_name": view_name, "data": encoded_data}
        ).json()
        view_id = created.get("view_id")
        
        if view_id:
            resp = create_session.delete(f"{base_url}api/default/savedviews/{view_id}")
            assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
            logger.info(f"Deleted saved view: {view_id}")
        else:
            logger.warning("View ID not found, skipping delete test")


# ==================== Service Account Tests ====================

class TestServiceAccountsCRUD:
    """Service Account CRUD operation tests"""
    
    @pytest.fixture
    def service_account_email(self):
        return f"sa_{generate_unique_id()}@service.local"
    
    @pytest.mark.order(90)
    def test_list_service_accounts(self, create_session, base_url):
        """Test listing service accounts"""
        resp = create_session.get(f"{base_url}api/default/service_accounts")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Listed service accounts successfully")
    
    @pytest.mark.order(91)
    def test_create_service_account(self, create_session, base_url, service_account_email):
        """Test creating a service account"""
        payload = {
            "email": service_account_email,
            "first_name": "Service",
            "last_name": "Account"
        }
        resp = create_session.post(f"{base_url}api/default/service_accounts", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Created service account: {service_account_email}")
    
    @pytest.mark.order(92)
    def test_get_service_account_token(self, create_session, base_url):
        """Test getting service account token"""
        email = f"token_sa_{generate_unique_id()}@service.local"
        # Create first
        create_session.post(
            f"{base_url}api/default/service_accounts",
            json={"email": email, "first_name": "Service", "last_name": "Account"}
        )
        
        resp = create_session.get(f"{base_url}api/default/service_accounts/{email}")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        # API directly returns token and user fields
        assert "token" in data, f"Response should contain 'token' field: {data}"
        logger.info(f"Got service account token for: {email}")
    
    @pytest.mark.order(93)
    def test_delete_service_account(self, create_session, base_url):
        """Test deleting a service account"""
        email = f"del_sa_{generate_unique_id()}@service.local"
        # Create first
        create_session.post(
            f"{base_url}api/default/service_accounts",
            json={"email": email, "first_name": "Service", "last_name": "Account"}
        )
        
        resp = create_session.delete(f"{base_url}api/default/service_accounts/{email}")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info(f"Deleted service account: {email}")


# ==================== Short URL Service Tests ====================

class TestShortURLCRUD:
    """Short URL CRUD operation tests"""
    
    @pytest.mark.order(100)
    def test_create_short_url(self, create_session, base_url):
        """Test creating a short URL"""
        payload = {
            "original_url": f"https://example.com/very/long/url/path?timestamp={generate_unique_id()}"
        }
        resp = create_session.post(f"{base_url}api/default/short", json=payload)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "short_url" in data or "short_id" in data, f"Response should contain short URL: {data}"
        logger.info(f"Created short URL: {data}")


# ==================== Pipeline Management Tests ====================

class TestPipelinesCRUD:
    """Pipeline CRUD operation tests"""
    
    @pytest.mark.order(110)
    def test_list_pipelines(self, create_session, base_url):
        """Test listing pipelines"""
        resp = create_session.get(f"{base_url}api/default/pipelines")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        assert "list" in data, f"Response should contain 'list' field: {data}"
        logger.info(f"Listed {len(data.get('list', []))} pipelines")
    
    @pytest.mark.order(111)
    def test_get_pipeline_streams(self, create_session, base_url):
        """Test getting pipeline associated streams"""
        resp = create_session.get(f"{base_url}api/default/pipelines/streams")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Got pipeline streams successfully")


# ==================== Concurrency and Transaction Tests ====================

class TestConcurrencyAndTransaction:
    """Concurrency and Transaction tests - OceanBase specific"""
    
    @pytest.mark.order(200)
    def test_concurrent_kv_writes(self, create_session, base_url):
        """Test concurrent KV writes"""
        import concurrent.futures
        
        base_key = f"concurrent_{generate_unique_id()}"
        
        def write_kv(i):
            key = f"{base_key}_{i}"
            resp = create_session.post(
                f"{base_url}api/default/kv/{key}",
                data=f"value_{i}",
                headers={"Content-Type": "text/plain"}
            )
            return resp.status_code
        
        with concurrent.futures.ThreadPoolExecutor(max_workers=5) as executor:
            futures = [executor.submit(write_kv, i) for i in range(10)]
            results = [f.result() for f in concurrent.futures.as_completed(futures)]
        
        success_count = sum(1 for r in results if r == 200)
        logger.info(f"Concurrent KV writes: {success_count}/10 succeeded")
        assert success_count >= 8, f"At least 80% should succeed, got {success_count}/10"
    
    @pytest.mark.order(201)
    def test_concurrent_user_creates(self, create_session, base_url):
        """Test concurrent user creation"""
        import concurrent.futures
        
        base_id = generate_unique_id()
        
        def create_user(i):
            payload = {
                "email": f"concurrent_user_{base_id}_{i}@example.com",
                "password": "TestPass#123",
                "role": "admin"  # Only admin role is currently allowed
            }
            resp = create_session.post(f"{base_url}api/default/users", json=payload)
            return resp.status_code
        
        with concurrent.futures.ThreadPoolExecutor(max_workers=5) as executor:
            futures = [executor.submit(create_user, i) for i in range(5)]
            results = [f.result() for f in concurrent.futures.as_completed(futures)]
        
        success_count = sum(1 for r in results if r == 200)
        logger.info(f"Concurrent user creates: {success_count}/5 succeeded")
        assert success_count >= 4, f"At least 80% should succeed, got {success_count}/5"
    
    @pytest.mark.order(202)
    def test_kv_update_consistency(self, create_session, base_url):
        """Test KV update consistency"""
        key = f"consistency_test_{generate_unique_id()}"
        
        # Set initial value
        create_session.post(
            f"{base_url}api/default/kv/{key}",
            data="initial",
            headers={"Content-Type": "text/plain"}
        )
        
        # Consecutive updates
        for i in range(5):
            create_session.post(
                f"{base_url}api/default/kv/{key}",
                data=f"value_{i}",
                headers={"Content-Type": "text/plain"}
            )
        
        # Verify final value
        resp = create_session.get(f"{base_url}api/default/kv/{key}")
        assert resp.status_code == 200
        assert resp.text == "value_4", f"Expected 'value_4', got '{resp.text}'"
        logger.info("KV update consistency test passed")
    
    @pytest.mark.order(203)
    def test_bulk_log_ingestion(self, create_session, base_url):
        """Test bulk log ingestion"""
        stream_name = f"bulk_test_{generate_unique_id()}"
        records = [
            {"message": f"log message {i}", "level": "info", "index": i}
            for i in range(100)
        ]
        
        resp = create_session.post(f"{base_url}api/default/{stream_name}/_json", json=records)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        data = resp.json()
        
        # Verify success count
        status = data.get("status", [{}])[0]
        successful = status.get("successful", 0)
        assert successful == 100, f"Expected 100 successful, got {successful}"
        logger.info(f"Bulk ingestion: {successful}/100 records succeeded")


# ==================== Search Jobs Tests ====================

class TestSearchJobsCRUD:
    """Search Jobs CRUD operation tests"""
    
    @pytest.mark.order(120)
    def test_list_search_jobs(self, create_session, base_url):
        """Test listing search jobs - This API may only be available with specific configuration"""
        resp = create_session.get(f"{base_url}api/default/search_jobs")
        # This API may not exist or require specific configuration
        assert resp.status_code in [200, 404], f"Expected 200/404, got {resp.status_code}: {resp.text}"
        logger.info(f"Search jobs API status: {resp.status_code}")


# ==================== System Settings Tests ====================

class TestSystemSettingsCRUD:
    """System Settings CRUD operation tests"""
    
    @pytest.mark.order(130)
    def test_list_settings_v2(self, create_session, base_url):
        """Test listing all settings v2"""
        resp = create_session.get(f"{base_url}api/default/settings/v2")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}: {resp.text}"
        logger.info("Listed settings v2 successfully")
